use crate::workspace::error::Error;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
};

pub(super) const MAX_OUTPUT_BYTES: usize = 100 * 1024;
const READ_CHUNK_BYTES: usize = 4096;

pub(super) struct Captured {
    pub(super) success: bool,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

pub(super) async fn capture(mut command: Command, deadline: Duration) -> Result<Captured, Error> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let result = tokio::time::timeout(deadline, collect(&mut child))
        .await
        .unwrap_or(Err(Error::ChecksTimeout));
    if result.is_err() {
        reap(&mut child).await?;
    }
    result
}

async fn collect(child: &mut Child) -> Result<Captured, Error> {
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let (status, stdout, stderr) = tokio::try_join!(
        async { child.wait().await.map_err(Error::from) },
        bounded(stdout),
        bounded(stderr)
    )?;
    Ok(Captured {
        success: status.success(),
        stdout,
        stderr,
    })
}

async fn bounded(mut reader: impl AsyncRead + Unpin) -> Result<String, Error> {
    let mut output = Vec::new();
    let mut buffer = [0; READ_CHUNK_BYTES];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        if output.len() + count > MAX_OUTPUT_BYTES / 2 {
            return Err(Error::ChecksOutputLimit);
        }
        output.extend_from_slice(&buffer[..count]);
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

async fn reap(child: &mut Child) -> Result<(), Error> {
    if child.try_wait()?.is_none() {
        kill(child).await?;
    }
    child.wait().await?;
    Ok(())
}

async fn kill(child: &mut Child) -> Result<(), Error> {
    let Err(error) = child.kill().await else {
        return Ok(());
    };
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    Err(error.into())
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
