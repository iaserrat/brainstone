//! The gate: format, lint, tests, doctests, and docs, fail-fast.
//!
//! This is the gate's source of truth. `AGENTS.md` names it rather than
//! restating it, so a new step is added in one place.

use crate::run::{self, Outcome};

pub(crate) fn run() -> Outcome {
    run::cargo(&["fmt", "--all", "--", "--check"])?;
    run::cargo(&[
        "clippy",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ])?;
    run::cargo(&["test", "--all-targets", "--all-features"])?;
    run::cargo(&["test", "--doc", "--all-features"])?;
    run::cargo_with(
        &["doc", "--no-deps", "--all-features"],
        "RUSTDOCFLAGS",
        "-D warnings",
    )
}
