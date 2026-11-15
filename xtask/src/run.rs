//! Child processes: echo the command, inherit the terminal, stop on the first
//! non-zero status so a failing step is never followed by a passing one.

use std::fmt::Write as _;
use std::process::{Command, ExitCode};

/// A task either ran every command it owns or stopped at the first failure.
pub(crate) type Outcome = Result<(), ()>;

pub(crate) fn outcome(result: Outcome) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

/// Runs `cargo` with `args`.
pub(crate) fn cargo(args: &[&str]) -> Outcome {
    command("cargo", args, &[])
}

/// Runs `cargo` with one environment variable set for the child.
pub(crate) fn cargo_with(args: &[&str], name: &str, value: &str) -> Outcome {
    command("cargo", args, &[(name, value)])
}

/// Runs `program`, echoing it first, and fails when it exits non-zero.
pub(crate) fn command(program: &str, args: &[&str], environment: &[(&str, &str)]) -> Outcome {
    println!("$ {}", describe(program, args, environment));
    let status = Command::new(program)
        .args(args)
        .envs(environment.iter().copied())
        .status()
        .map_err(|error| eprintln!("{program}: {error}"))?;
    if status.success() { Ok(()) } else { Err(()) }
}

/// The command line as a shell would need it, so a copied line reproduces it.
fn describe(program: &str, args: &[&str], environment: &[(&str, &str)]) -> String {
    let mut line = String::new();
    for (name, value) in environment {
        write!(line, "{name}={value} ").expect("writing to a String cannot fail");
    }
    line.push_str(program);
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}
