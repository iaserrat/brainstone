use crate::workspace::{Workspace, error::Error};
use brainstone::{Tool, ToolError, typed_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunChecks {}

#[derive(Debug, Serialize)]
struct CheckResult {
    command: &'static str,
    stdout: String,
    stderr: String,
}

const TIMEOUT: Duration = Duration::from_secs(30);
const ENVELOPE_RESERVE_BYTES: usize = 1024;

pub(super) fn tool(workspace: Workspace) -> Result<impl Tool, ToolError> {
    typed_tool(
        "RunChecks",
        "Run cargo test --offline --locked --lib -- --test-threads=1 in this trusted workspace; this executes repository code.",
        move |_: RunChecks, _| {
            let workspace = workspace.clone();
            async move { checks(&workspace).await }
        },
    )
}

async fn checks(workspace: &Workspace) -> Result<CheckResult, ToolError> {
    let output = super::process::capture(command(workspace), TIMEOUT).await?;
    let result = CheckResult {
        command: "cargo test --offline --locked --lib -- --test-threads=1",
        stdout: output.stdout,
        stderr: output.stderr,
    };
    let encoded = serde_json::to_string(&result).expect("check output contains only strings");
    if encoded.len() > super::process::MAX_OUTPUT_BYTES - ENVELOPE_RESERVE_BYTES {
        return Err(Error::ChecksOutputLimit.into());
    }
    if !output.success {
        return Err(Error::ChecksFailed(encoded).into());
    }
    Ok(result)
}

fn command(workspace: &Workspace) -> Command {
    let mut command = Command::new("cargo");
    command
        .args([
            "test",
            "--offline",
            "--locked",
            "--lib",
            "--",
            "--test-threads=1",
        ])
        .current_dir(workspace.root())
        .env_remove("CARGO_TARGET_DIR")
        .stdin(Stdio::null());
    command
}
