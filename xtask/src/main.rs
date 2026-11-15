//! The workspace tasks. `cargo xtask <task>` runs one; see the usage line.

mod audit;
mod gate;
mod postgres;
mod run;

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("gate") => run::outcome(gate::run()),
        Some("audit") => run::outcome(audit::run()),
        Some("postgres-test") => run::outcome(postgres::run()),
        Some(unknown) => usage(Some(unknown)),
        None => usage(None),
    }
}

fn usage(unknown: Option<&str>) -> ExitCode {
    if let Some(name) = unknown {
        eprintln!("unknown task: {name}");
    }
    eprintln!("usage: cargo xtask <gate|audit|postgres-test>");
    ExitCode::FAILURE
}
