//! The facade must work without the configured deployment stack.

use crate::run::{self, Outcome};

pub(crate) fn run() -> Outcome {
    core("clippy", &["--all-targets", "--", "-D", "warnings"])?;
    core("test", &["--all-targets"])?;
    core("test", &["--doc"])?;
    run::cargo_with(
        &[
            "doc",
            "-p",
            "brainstone",
            "--no-deps",
            "--no-default-features",
        ],
        "RUSTDOCFLAGS",
        "-D warnings",
    )?;
    adapters()
}

fn core(command: &str, options: &[&str]) -> Outcome {
    let mut args = vec![command, "-p", "brainstone", "--no-default-features"];
    args.extend_from_slice(options);
    run::cargo(&args)
}

fn adapters() -> Outcome {
    for feature in [
        "anthropic",
        "openai",
        "mcp",
        "memory",
        "policy",
        "skills",
        "subagents",
        "surface",
        "telemetry",
    ] {
        run::cargo(&[
            "check",
            "-p",
            "brainstone",
            "--no-default-features",
            "--features",
            feature,
        ])?;
    }
    Ok(())
}
