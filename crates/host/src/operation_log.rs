use agent::Outcome;
use runtime::Operation;

pub(crate) fn record(operation: Option<&Operation>) {
    let Some(Operation {
        id,
        outcome: Some(Outcome::Failed { code, message }),
        ..
    }) = operation
    else {
        return;
    };
    tracing::warn!(name: "brainstone.operation.failed",
        operation_id = id.get(),
        code,
        error = message,
        "operation failed"
    );
}
