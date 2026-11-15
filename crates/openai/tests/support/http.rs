#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
#![allow(
    dead_code,
    reason = "shared HTTP fixtures used by separate protocol test binaries"
)]
use agent::{Event, Sink};
use serde_json::Value;
use std::fmt::Write as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Clone)]
pub(crate) struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: ReplyBody,
    pub content_type: &'static str,
}

#[derive(Clone)]
pub(crate) enum ReplyBody {
    Json(Value),
    Raw(&'static str),
    Owned(String),
}

impl ReplyBody {
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Json(value) => value.to_string(),
            Self::Raw(value) => (*value).to_owned(),
            Self::Owned(value) => value.clone(),
        }
    }
}

impl Reply {
    pub(crate) fn ok(body: Value) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: ReplyBody::Json(body),
            content_type: "application/json",
        }
    }

    pub(crate) fn sse(body: String) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: ReplyBody::Owned(body),
            content_type: "text/event-stream",
        }
    }
}

#[derive(Default)]
pub(crate) struct Events(pub Vec<Event>);

impl Sink for Events {
    type Error = &'static str;

    async fn emit(&mut self, event: Event) -> Result<(), Self::Error> {
        self.0.push(event);
        Ok(())
    }
}

pub(crate) struct Captured {
    pub body: Value,
    pub authorization: String,
}

pub(crate) async fn serve_once(response: Value) -> (String, tokio::task::JoinHandle<Captured>) {
    serve_reply(Reply::ok(response)).await
}

pub(crate) async fn serve_reply(reply: Reply) -> (String, tokio::task::JoinHandle<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        write_reply(&mut stream, &reply).await;
        request
    });
    (format!("http://{address}/v1/responses"), task)
}

pub(crate) async fn serve_many(
    replies: Vec<Reply>,
) -> (String, tokio::task::JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            write_reply(&mut stream, &reply).await;
        }
        requests
    });
    (format!("http://{address}/v1/responses"), task)
}

async fn write_reply(stream: &mut tokio::net::TcpStream, reply: &Reply) {
    let body = reply.body.text();
    let extra = reply
        .headers
        .iter()
        .fold(String::new(), |mut output, (name, value)| {
            write!(output, "{name}: {value}\r\n").unwrap();
            output
        });
    let head = format!(
        "HTTP/1.1 {} Test\r\ncontent-type: {}\r\ncontent-length: {}\r\n{extra}connection: close\r\n\r\n",
        reply.status,
        reply.content_type,
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Captured {
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let count = stream.read(&mut chunk).await.unwrap();
        bytes.extend_from_slice(&chunk[..count]);
        let Some((start, length)) = body_bounds(&bytes) else {
            continue;
        };
        if bytes.len() >= start + length {
            let body = serde_json::from_slice(&bytes[start..start + length]).unwrap();
            let head = std::str::from_utf8(&bytes[..start]).unwrap();
            let authorization = header(head, "authorization").unwrap_or_default();
            return Captured {
                body,
                authorization,
            };
        }
    }
}

fn header(request: &str, wanted: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case(wanted)
            .then(|| value.trim().to_owned())
    })
}

fn body_bounds(request: &[u8]) -> Option<(usize, usize)> {
    let text = std::str::from_utf8(request).ok()?;
    let start = text.find("\r\n\r\n")? + 4;
    let length = text.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .strip_prefix("content-length: ")?
            .parse()
            .ok()
    })?;
    Some((start, length))
}
