//! The opt-in PostgreSQL contract suite. Needs a database the tests may create
//! and drop objects in; every default target skips these cases.

use crate::run::{self, Outcome};

const VARIABLE: &str = "BRAINSTONE_TEST_DATABASE_URL";
const IGNORED: &str = "--ignored";

const RUNTIME: &[&str] = &[
    "test",
    "-p",
    "brainstone-runtime",
    "--test",
    "backend_contracts",
    "--test",
    "failure_boundaries",
    "--test",
    "steering",
    "--test",
    "postgres",
    "--test",
    "postgres_tools",
    "--test",
    "postgres_hot_paths",
    "--test",
    "turn_limits",
    "--test",
    "postgres_media",
    "--test",
    "identity",
    "--test",
    "approvals",
    "--test",
    "session_delete",
    "--",
    IGNORED,
    "--test-threads=1",
];

// Filter by the PostgreSQL case so the ignored subprocess fixture is not run directly.
const TOOL_CONTRACT: &[&str] = &[
    "test",
    "-p",
    "brainstone-runtime",
    "--test",
    "tool_contract_recovery",
    "postgres_tool_contract_recovers_committed_operation_evidence",
    "--",
    IGNORED,
];

const HOST: &[&str] = &[
    "test",
    "-p",
    "brainstone-host",
    "--test",
    "postgres",
    "--",
    IGNORED,
    "--test-threads=1",
];

const SUBAGENTS: &[&str] = &[
    "test",
    "-p",
    "brainstone-subagents",
    "--test",
    "output_timeout",
    "--",
    IGNORED,
    "--test-threads=1",
];

pub(crate) fn run() -> Outcome {
    if std::env::var(VARIABLE).unwrap_or_default().is_empty() {
        eprintln!("set {VARIABLE} to a database the suite may create objects in");
        return Err(());
    }
    run::cargo(RUNTIME)?;
    run::cargo(TOOL_CONTRACT)?;
    run::cargo(HOST)?;
    run::cargo(SUBAGENTS)
}
