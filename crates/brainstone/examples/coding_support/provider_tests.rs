//! The real provider adapter and live CLI path run against a local endpoint.
#![allow(clippy::too_many_lines)] // Request contracts and subprocess assertions.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[test]
fn live_run_approves_tools_and_terminal_retries_need_no_provider() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("project");
    super::demo::prepare(&root).unwrap();
    let (endpoint, server) = serve();
    let first = child(&run_args(&root), &endpoint);
    assert!(first.status.success(), "{first:?}");
    let captured = server.join().unwrap();
    assert_eq!(captured.len(), 4);
    for request in &captured {
        assert_eq!(request["model"], "coding-fixture");
        assert_eq!(request["text"]["format"]["type"], "json_schema");
        assert_eq!(request["text"]["format"]["name"], "coding_summary");
        assert_eq!(request["text"]["format"]["strict"], true);
        let names: Vec<_> = request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["ReadFile", "EditFile", "RunChecks"]);
    }
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        super::demo::after()
    );
    let approval = String::from_utf8_lossy(&first.stderr);
    assert_eq!(approval.matches("Approval:").count(), 2);
    assert!(approval.contains("\"name\":\"EditFile\""));
    assert!(approval.contains("\"name\":\"RunChecks\""));
    assert_eq!(answer(&first)["checks"], "passed");

    // The listener is closed. Repeating admission or reading the terminal ID
    // would fail if either path attempted another provider request.
    let duplicate = child(&run_args(&root), &endpoint);
    assert!(duplicate.status.success(), "{duplicate:?}");
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate=true"));
    let resumed = child(
        &[
            "resume".into(),
            root.display().to_string(),
            operation(&first),
        ],
        &endpoint,
    );
    assert!(resumed.status.success(), "{resumed:?}");
    for output in [&duplicate, &resumed] {
        assert_eq!(answer(output), answer(&first));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Approval:"));
    }
}

fn run_args(root: &Path) -> Vec<String> {
    vec![
        "run".into(),
        root.display().to_string(),
        "live-1".into(),
        super::profile::DEMO_TASK.into(),
    ]
}
fn child(args: &[String], endpoint: &str) -> Output {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "provider_tests::worker",
            "--nocapture",
        ])
        .env(
            "BRAINSTONE_CODING_PROVIDER_ARGS",
            serde_json::to_string(args).unwrap(),
        )
        .env("OPENAI_ENDPOINT", endpoint)
        .env("OPENAI_MODEL", "coding-fixture")
        .env("OPENAI_API_KEY", "local-test-key")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"y\ny\n").unwrap();
    child.wait_with_output().unwrap()
}
fn answer(output: &Output) -> Value {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .expect("JSON final answer")
}
fn operation(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .find_map(|line| line.strip_prefix("operation="))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn serve() -> (String, JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/responses", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut captured = Vec::new();
        for response in responses() {
            let mut stream = accept(&listener);
            captured.push(request(&mut stream));
            reply(&mut stream, response);
        }
        captured
    });
    (endpoint, server)
}
#[test]
fn accepted_stream_waits_for_request_bytes_arriving_after_accept() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (ready, accepted) = std::sync::mpsc::channel();
    let writer = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        accepted.recv().unwrap();
        thread::sleep(Duration::from_millis(100));
        stream
            .write_all(b"POST /responses HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
    });
    let mut stream = accept(&listener);
    ready.send(()).unwrap();
    assert_eq!(request(&mut stream), json!({}));
    writer.join().unwrap();
}

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // macOS can inherit the listener's nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "expected another provider request"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}
fn request(stream: &mut TcpStream) -> Value {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 16 * 1024, "request header limit");
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8(bytes).unwrap();
    assert!(header.starts_with("POST /responses HTTP/1.1"));
    let length = header
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    assert!(length <= 256 * 1024, "request body limit");
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}
fn reply(stream: &mut TcpStream, response: Value) {
    let event =
        json!({"type":"response.completed","response":{"status":"completed","output":[response]}});
    let body = format!("data: {event}\n\n");
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    stream.flush().unwrap();
}
fn responses() -> [Value; 4] {
    [
        call("ReadFile", json!({"path":"src/lib.rs"})),
        call("EditFile", super::demo::edit_args()),
        call("RunChecks", json!({})),
        json!({"type":"message","content":[{"type":"output_text","text":json!({"summary":"Fixed add and ran the tests.","files":["src/lib.rs"],"checks":"passed"}).to_string()}]}),
    ]
}
fn call(name: &str, arguments: Value) -> Value {
    json!({"type":"function_call","call_id":name,"name":name,"arguments":arguments.to_string()})
}

#[tokio::test]
#[ignore = "live CLI subprocess fixture"]
async fn worker() {
    let args: Vec<String> =
        serde_json::from_str(&std::env::var("BRAINSTONE_CODING_PROVIDER_ARGS").unwrap()).unwrap();
    super::execute(super::input::parse(args.into_iter()).unwrap())
        .await
        .unwrap();
}
