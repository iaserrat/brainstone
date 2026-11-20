//! The supply-chain audit: unused dependencies, then deny.toml's advisory,
//! license, ban, and source policy.
//!
//! The tools are named directly. `cargo machete` hands the subcommand name to
//! cargo-machete as a path when it is spawned from a cargo-run child, which
//! reports a missing directory called `machete`.

use crate::run::{self, Outcome};

pub(crate) fn run() -> Outcome {
    run::command("cargo-machete", &[], &[])?;
    run::command("cargo-deny", &["check"], &[])
}
