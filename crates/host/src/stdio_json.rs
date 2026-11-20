//! One private-stdio exchange with an operator-configured executable: write
//! one JSON document to its stdin, read one JSON object line from its stdout.
//! Model selection uses it and maps `Failed` to its own stable code.
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

const MAX_INPUT: usize = 65_536;
const MAX_OUTPUT: u64 = 16_384;

/// The exchange failed: the child would not start, broke a size limit or the
/// deadline, or did not answer with one JSON object line.
#[derive(Debug)]
pub struct Failed;

/// Spawns `program`, writes `input`, and reads the first stdout line as a JSON
/// object within `deadline`. The child is returned still running so a caller
/// may wait for it; dropping it kills it.
///
/// # Errors
///
/// Returns `Failed` when the child cannot start, the exchange exceeds a size
/// limit or `deadline`, or the first line is not a newline-terminated object.
pub async fn exchange(
    program: &str,
    args: &[String],
    input: &Value,
    deadline: Duration,
) -> Result<(Value, Child), Failed> {
    let input = serde_json::to_vec(input).map_err(|_| Failed)?;
    if input.len() > MAX_INPUT {
        return Err(Failed);
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| Failed)?;
    let output = tokio::time::timeout(deadline, read(&mut child, &input))
        .await
        .map_err(|_| Failed)??;
    Ok((output, child))
}

async fn read(child: &mut Child, input: &[u8]) -> Result<Value, Failed> {
    let mut stdin = child.stdin.take().ok_or(Failed)?;
    stdin.write_all(input).await.map_err(|_| Failed)?;
    drop(stdin);
    let stdout = child.stdout.take().ok_or(Failed)?;
    let mut line = String::new();
    BufReader::new(stdout.take(MAX_OUTPUT))
        .read_line(&mut line)
        .await
        .map_err(|_| Failed)?;
    if !line.ends_with('\n') {
        return Err(Failed);
    }
    let output: Value = serde_json::from_str(&line).map_err(|_| Failed)?;
    if !output.is_object() {
        return Err(Failed);
    }
    Ok(output)
}
