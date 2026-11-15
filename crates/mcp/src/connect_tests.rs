//! Handshake behavior against a legacy server that mishandles newer protocol
//! versions: it rejects `server/discover`, and negotiates the `initialize`
//! body down but rejects every request carrying the session it issued
//! (GitHub's hosted MCP, 2026-08).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmcp::model::ProtocolVersion;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::{Server, Transport};
use crate::interaction::Decline;

/// A streamable-HTTP server that honors exactly one protocol version, below
/// the newest `initialize` revision. Requesting any other version still
/// returns 200 and a session id, but the session is poisoned: every request
/// carrying it gets 400 "invalid session". `requested` records each
/// handshake: `discover`, or the version an `initialize` offered.
struct Mock {
    known: String,
    requested: Mutex<Vec<String>>,
    valid: Mutex<Vec<String>>,
    /// Never answer `server/discover`, as a server that drops unknown methods.
    silent_discover: bool,
}

struct Request {
    method: String,
    session: Option<String>,
    body: Vec<u8>,
}

type Reply = (&'static str, Option<String>, String);

/// The status of a request the mock never answers.
const SILENT: &str = "silent";

impl Mock {
    fn new() -> Self {
        Self {
            known: ProtocolVersion::V_2025_06_18.as_str().to_owned(),
            requested: Mutex::new(Vec::new()),
            valid: Mutex::new(Vec::new()),
            silent_discover: false,
        }
    }

    fn reply(&self, request: &Request) -> Reply {
        if request.method != "POST" {
            return ("405 Method Not Allowed", None, String::new());
        }
        let message: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        if message["method"] == "initialize" {
            return self.initialize(&message);
        }
        if message["method"] == "server/discover" {
            self.requested.lock().unwrap().push("discover".to_owned());
            if self.silent_discover {
                return (SILENT, None, String::new());
            }
        }
        let valid = request
            .session
            .as_ref()
            .is_some_and(|session| self.valid.lock().unwrap().contains(session));
        if !valid {
            return ("400 Bad Request", None, "invalid session".into());
        }
        if message["method"] == "tools/list" {
            let body = json!({"jsonrpc": "2.0", "id": message["id"], "result": {"tools": []}});
            return ("200 OK", None, body.to_string());
        }
        ("202 Accepted", None, String::new())
    }

    fn initialize(&self, message: &Value) -> Reply {
        let requested = message["params"]["protocolVersion"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let session = format!("session-{}", self.requested.lock().unwrap().len());
        if requested == self.known {
            self.valid.lock().unwrap().push(session.clone());
        }
        self.requested.lock().unwrap().push(requested);
        let body = json!({"jsonrpc": "2.0", "id": message["id"], "result": {
            "protocolVersion": self.known,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "mock", "version": "0"},
        }});
        ("200 OK", Some(session), body.to_string())
    }
}

async fn spawn_mock() -> (String, Arc<Mock>) {
    spawn(Mock::new()).await
}

async fn spawn(mock: Mock) -> (String, Arc<Mock>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("mock binds");
    let url = format!("http://{}", listener.local_addr().expect("mock address"));
    let mock = Arc::new(mock);
    let accepting = mock.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve_connection(stream, accepting.clone()));
        }
    });
    (url, mock)
}

async fn serve_connection(mut stream: TcpStream, mock: Arc<Mock>) {
    while let Some(request) = read_request(&mut stream).await {
        let (status, session, body) = mock.reply(&request);
        if status == SILENT {
            std::future::pending::<()>().await;
        }
        respond(&mut stream, status, session.as_deref(), &body).await;
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buffer = Vec::new();
    let head_end = read_until_headers(stream, &mut buffer).await?;
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let length: usize = header(&head, "content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    while buffer.len() < head_end + length {
        if read_chunk(stream, &mut buffer).await? == 0 {
            return None;
        }
    }
    Some(Request {
        method: head.split(' ').next().unwrap_or_default().to_owned(),
        session: header(&head, "mcp-session-id"),
        body: buffer[head_end..head_end + length].to_vec(),
    })
}

async fn read_until_headers(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Option<usize> {
    loop {
        let end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
        if let Some(end) = end {
            return Some(end + 4);
        }
        if read_chunk(stream, buffer).await? == 0 {
            return None;
        }
    }
}

async fn read_chunk(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Option<usize> {
    let mut chunk = [0_u8; 1024];
    let count = stream.read(&mut chunk).await.ok()?;
    buffer.extend_from_slice(&chunk[..count]);
    Some(count)
}

fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

async fn respond(stream: &mut TcpStream, status: &str, session: Option<&str>, body: &str) {
    let session = session.map_or(String::new(), |id| format!("Mcp-Session-Id: {id}\r\n"));
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{session}\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.ok();
}

fn mock_server(url: String) -> Server {
    Server {
        access: agent::AccessPolicy::default(),
        name: "mock".into(),
        transport: Transport::Http {
            url,
            bearer_token: None,
            headers: BTreeMap::new(),
        },
        required: true,
        startup_timeout: Duration::from_secs(10),
        tool_timeout: Duration::from_secs(5),
        enabled_tools: None,
    }
}

#[tokio::test]
async fn handshake_walks_down_when_a_server_rejects_sessions_for_newer_versions() {
    let (url, mock) = spawn_mock().await;
    let mut client = crate::connect::connect(&mock_server(url), Arc::new(Decline))
        .await
        .expect("handshake falls back to the version the server honors");
    let tools = client.tools().await.expect("negotiated session is usable");
    assert!(tools.is_empty());

    // The newest revision has no `initialize`, so it is offered by discover;
    // the walk then offers each `initialize` revision, newest first.
    let requested = mock.requested.lock().unwrap().clone();
    assert_eq!(
        requested,
        [
            "discover",
            ProtocolVersion::LATEST_WITH_INITIALIZE.as_str(),
            mock.known.as_str(),
        ],
        "the walk must start at the newest revision and land on the honored one"
    );
    client.close().await.expect("client closes");
}

/// A server that never answers `server/discover` costs the discover attempt
/// at most half the startup budget, not all of it: the walk goes on to
/// `initialize` and connects.
#[tokio::test]
async fn a_server_that_never_answers_discover_still_connects_by_initialize() {
    let mock = Mock {
        silent_discover: true,
        ..Mock::new()
    };
    let (url, mock) = spawn(mock).await;
    let mut server = mock_server(url);
    server.startup_timeout = Duration::from_secs(2);

    let mut client = crate::connect::connect(&server, Arc::new(Decline))
        .await
        .expect("the walk continues past an unanswered discover");

    let requested = mock.requested.lock().unwrap().clone();
    assert_eq!(requested.first().map(String::as_str), Some("discover"));
    assert_eq!(
        requested.last().map(String::as_str),
        Some(mock.known.as_str())
    );
    client.close().await.expect("client closes");
}
