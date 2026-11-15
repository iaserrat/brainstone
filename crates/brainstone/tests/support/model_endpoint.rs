use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const HEADER_LIMIT: usize = 65536;
const READ_BUFFER: usize = 4096;
const HEADER_END: &[u8] = b"\r\n\r\n";

/// One inference response; closing the listener makes a later network call fail.
/// The handle yields the request head and body.
pub(crate) async fn serve_once(value: Value) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/inference", listener.local_addr().unwrap());
    let captured = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let head = request(&mut stream).await;
        let body = value.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        head
    });
    (endpoint, captured)
}

async fn request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0; READ_BUFFER];
    while !bytes
        .windows(HEADER_END.len())
        .any(|part| part == HEADER_END)
    {
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(
            count > 0 && bytes.len() < HEADER_LIMIT,
            "incomplete request headers"
        );
        bytes.extend_from_slice(&chunk[..count]);
    }
    let end = bytes
        .windows(HEADER_END.len())
        .position(|part| part == HEADER_END)
        .unwrap();
    let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
    let mut body = bytes[end + HEADER_END.len()..].to_vec();
    drain(stream, &head, &mut body).await;
    format!("{head}\r\n\r\n{}", String::from_utf8(body).unwrap())
}

async fn drain(stream: &mut TcpStream, head: &str, body: &mut Vec<u8>) {
    let length = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map_or_else(usize::default, |(_, value)| {
            value.trim().parse::<usize>().unwrap()
        });
    let remaining = u64::try_from(length.saturating_sub(body.len())).unwrap();
    let copied = stream.take(remaining).read_to_end(body).await.unwrap();
    assert_eq!(u64::try_from(copied).unwrap(), remaining);
}
