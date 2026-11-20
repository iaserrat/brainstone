use crate::{Outcome, Step, Thread};

/// A successful final answer lacks a required committed tool result.
pub const CODE_TOOL_REQUIREMENT_UNMET: &str = "tool_requirement_unmet";
/// Recovery did not supply authoritative current-operation effect evidence.
pub const CODE_TOOL_EVIDENCE_UNAVAILABLE: &str = "tool_evidence_unavailable";

pub(super) fn validate(thread: &Thread, step: Step) -> Step {
    if !matches!(step, Step::Done(Outcome::Completed { .. })) {
        return step;
    }
    let Some(contract) = thread.profile().tool_contract() else {
        return step;
    };
    if contract.required_tools().is_empty() {
        return step;
    }
    let Some(successes) = thread.tool_successes() else {
        return Step::Done(Outcome::failed(
            CODE_TOOL_EVIDENCE_UNAVAILABLE,
            "authoritative operation tool evidence was not restored",
        ));
    };
    required(contract.required_tools(), successes, step)
}

fn required(names: &[String], successes: &[String], step: Step) -> Step {
    let missing: Vec<&str> = names
        .iter()
        .filter(|name| !successes.contains(name))
        .map(String::as_str)
        .collect();
    if missing.is_empty() {
        return step;
    }
    Step::Done(Outcome::failed(
        CODE_TOOL_REQUIREMENT_UNMET,
        format!(
            "required tools did not commit successful results: {}",
            missing.join(", ")
        ),
    ))
}
