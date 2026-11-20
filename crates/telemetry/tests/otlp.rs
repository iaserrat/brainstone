#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! A real local OTLP receiver decodes requests sent by the production initializer.
// Scenario assertions deliberately keep both signals and correlation together.
#![allow(clippy::too_many_lines)]
use opentelemetry_proto::tonic::collector::{
    metrics::v1::ExportMetricsServiceRequest, trace::v1::ExportTraceServiceRequest,
};
use prost::Message as _;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn local_receiver_decodes_runtime_traces_metrics_and_correlated_safe_logs() {
    let (output, requests) = scenario("always_on", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let traces = traces(&requests);
    assert!(traces.contains("operation"));
    assert!(traces.contains("model"));
    assert!(traces.contains("service.name"));
    assert!(traces.contains("receiver-test"));
    assert!(traces.contains("deployment.environment.name"));
    assert!(traces.contains("staging"));
    assert!(traces.contains("service.version"));
    assert!(traces.contains("test-version"));
    let metrics = metrics(&requests);
    assert!(metrics.contains("brainstone.work.duration"));
    assert!(metrics.contains("brainstone.work.finished"));
    assert!(!metrics.contains("operation_id"));
    let logs = String::from_utf8(output.stderr).unwrap();
    assert!(!logs.contains("SENSITIVE"));
    let logs: Vec<serde_json::Value> = logs
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let correlated = logs
        .iter()
        .find(|log| log["kind"] == "operation" && log["outcome"] == "ok")
        .expect("operation log");
    assert_eq!(correlated["trace_id"].as_str().unwrap().len(), 32);
    assert_eq!(correlated["span_id"].as_str().unwrap().len(), 16);
    assert!(
        correlated["dd.trace_id"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .is_ok()
    );
    assert_eq!(correlated["operation_id"], 1);
    assert!(!format!("{traces}{metrics}{logs:?}").contains("SENSITIVE"));
}

#[test]
fn sampling_and_log_filtering_do_not_disable_metrics() {
    let (output, requests) = scenario("always_off", false);
    assert!(output.status.success());
    assert_eq!(traces(&requests), "[]");
    assert!(metrics(&requests).contains("brainstone.work.finished"));
}

#[test]
fn receiver_failure_does_not_fail_or_delay_runtime_execution() {
    let (output, _) = scenario("always_on", true);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("telemetry_trace_export_failed")
    );
}

#[test]
fn slow_receiver_saturates_without_blocking_work_and_shutdown_is_bounded() {
    let started = Instant::now();
    let (output, _) = scenario("saturation", false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(started.elapsed() < Duration::from_secs(12));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("BatchSpanProcessor.SpanDroppingStarted")
    );
}

#[deny(clippy::too_many_lines)]
fn command(endpoint: &str, mode: &str) -> Child {
    let (sampler, flood, filter) = match mode {
        "saturation" => ("always_on", "1", "warn"),
        "always_off" => ("always_off", "0", "off"),
        _ => (mode, "0", "info"),
    };
    Command::new(std::env::current_exe().unwrap())
        .env_clear()
        .args(["--ignored", "--exact", "child", "--nocapture"])
        .envs([
            ("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint),
            ("OTEL_SERVICE_NAME", "receiver-test"),
            (
                "OTEL_RESOURCE_ATTRIBUTES",
                "deployment.environment.name=staging,service.version=test-version",
            ),
            ("OTEL_TRACES_SAMPLER", sampler),
            ("BRAINSTONE_TEST_FLOOD", flood),
            ("RUST_LOG", filter),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}
#[deny(clippy::too_many_lines)]
fn scenario(sampler: &str, fail: bool) -> (std::process::Output, Vec<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = command(
        &format!("http://{}", listener.local_addr().unwrap()),
        sampler,
    );
    let started = Instant::now();
    let mut requests = Vec::new();
    while child.try_wait().unwrap().is_none() {
        assert!(started.elapsed() < Duration::from_secs(20));
        match listener.accept() {
            Ok((stream, _)) => requests.push(receive(stream, fail, sampler == "saturation")),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("{error}"),
        }
    }
    (child.wait_with_output().unwrap(), requests)
}
#[deny(clippy::too_many_lines)]
fn receive(mut stream: TcpStream, fail: bool, slow: bool) -> (String, Vec<u8>) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let header = read_header(&mut stream);
    let mut body = vec![0; content_length(&header)];
    stream.read_exact(&mut body).unwrap();
    if slow {
        std::thread::sleep(Duration::from_secs(4));
    }
    respond(&mut stream, fail, slow);
    (header, body)
}
#[deny(clippy::too_many_lines)]
fn content_length(header: &str) -> usize {
    header
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .unwrap()
        .parse()
        .unwrap()
}
#[deny(clippy::too_many_lines)]
fn respond(stream: &mut TcpStream, fail: bool, slow: bool) {
    let status = if fail {
        "503 Service Unavailable"
    } else {
        "200 OK"
    };
    let response = write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: application/x-protobuf\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    if !slow {
        response.unwrap();
    }
}

#[deny(clippy::too_many_lines)]
fn read_header(stream: &mut TcpStream) -> String {
    let mut data = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        data.push(byte[0]);
        if data.ends_with(b"\r\n\r\n") {
            break String::from_utf8(data.clone()).unwrap();
        }
    }
}

#[deny(clippy::too_many_lines)]
fn traces(requests: &[(String, Vec<u8>)]) -> String {
    format!(
        "{:?}",
        requests
            .iter()
            .filter(|(head, _)| head.starts_with("POST /v1/traces "))
            .map(|(_, body)| { ExportTraceServiceRequest::decode(body.as_slice()).unwrap() })
            .collect::<Vec<_>>()
    )
}
#[deny(clippy::too_many_lines)]
fn metrics(requests: &[(String, Vec<u8>)]) -> String {
    format!(
        "{:?}",
        requests
            .iter()
            .filter(|(head, _)| head.starts_with("POST /v1/metrics "))
            .map(|(_, body)| { ExportMetricsServiceRequest::decode(body.as_slice()).unwrap() })
            .collect::<Vec<_>>()
    )
}

#[tokio::test]
#[ignore = "isolated global initializer fixture"]
async fn child() {
    let telemetry = brainstone_telemetry::init().unwrap();
    let started = Instant::now();
    let runtime = runtime::Runtime::new(runtime::MemoryStore::default());
    let prompt = runtime::Prompt::new(
        runtime::Address::new("test", "SENSITIVE-CHANNEL", "SENSITIVE-THREAD").unwrap(),
        "SENSITIVE-KEY",
        "SENSITIVE-SYSTEM",
        "SENSITIVE-PROMPT",
    )
    .unwrap();
    runtime.accept(&prompt).await.unwrap();
    runtime
        .run_next(&Answer, &agent::Registry::default())
        .await
        .unwrap()
        .unwrap();
    let hostile = tracing::info_span!(target: "brainstone::telemetry", "work", kind = "tool", outcome = "failed", prompt = "SENSITIVE-PROMPT", error = "SENSITIVE-ERROR", otel.status_description = "SENSITIVE-DESCRIPTION");
    hostile.in_scope(|| tracing::info!(prompt = "SENSITIVE-PROMPT", "SENSITIVE-EVENT"));
    drop(hostile);
    tracing::error!(
        error = "SENSITIVE-ERROR",
        args = "SENSITIVE-ARGS",
        "SENSITIVE-MESSAGE"
    );
    if std::env::var("BRAINSTONE_TEST_FLOOD").as_deref() == Ok("1") {
        for _ in 0..5_000 {
            drop(
                tracing::info_span!(target: "brainstone::telemetry", "work", kind = "provider", outcome = "ok"),
            );
        }
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    telemetry.shutdown().await;
}
struct Answer;
impl agent::Model for Answer {
    async fn generate(&self, _: &agent::Thread, _: &[serde_json::Value]) -> agent::ModelResponse {
        agent::ModelResponse {
            reasoning: vec![],
            text: "SENSITIVE-COMPLETION".into(),
            tool_calls: vec![],
            stop: agent::StopReason::EndTurn,
            model: None,
        }
    }
}
