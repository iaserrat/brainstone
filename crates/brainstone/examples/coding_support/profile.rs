use brainstone::{ExecutionProfile, OutputSchema, ToolContract};
use serde_json::json;

const MAX_MODEL_TURNS: usize = 12;
const ROOT_DEPTH: u8 = 0;

pub(crate) const SYSTEM: &str = "You fix Rust source in the supplied workspace. Read files before editing. EditFile replaces the complete file only when expected matches. Request RunChecks in a separate turn after the final edit. Never batch RunChecks with EditFile. It runs cargo test --offline --locked --lib -- --test-threads=1. Editing is closed after successful checks. Report only committed effects. Finish with JSON containing summary, files, and checks. Never claim passed checks after a tool failure.";
pub(crate) const DEMO_TASK: &str = "Fix add in src/lib.rs so the existing test passes. Read the file, apply the exact fix, and run checks.";

pub(crate) fn execution() -> Result<ExecutionProfile, super::Error> {
    let contract = ToolContract::only(["ReadFile", "EditFile", "RunChecks"])?
        .require_approval(["EditFile", "RunChecks"])?
        .require_success(["EditFile", "RunChecks"])?;
    let schema = OutputSchema::new(
        "coding_summary",
        json!({
            "type":"object", "properties":{
                "summary":{"type":"string"},
                "files":{"type":"array","items":{"type":"string"}},
                "checks":{"type":"string","enum":["passed"]}
            }, "required":["summary","files","checks"], "additionalProperties":false
        }),
    )?;
    Ok(
        ExecutionProfile::new(None, None, MAX_MODEL_TURNS, ROOT_DEPTH)?
            .with_tool_contract(contract)
            .with_output_schema(schema),
    )
}
