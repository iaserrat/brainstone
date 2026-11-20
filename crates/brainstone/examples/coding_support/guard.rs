//! Edits stop after successful checks; mixed edit/check batches are rejected.
use crate::workspace::error::Error;
use brainstone::runtime::{DurableStore, OperationId};
use brainstone::{
    ToolError, ToolInvocation,
    agent::{Event, Message},
};

const PAGE_SIZE: u16 = 100;

pub(super) async fn before_edit(
    store: &DurableStore,
    invocation: &ToolInvocation,
) -> Result<(), ToolError> {
    let context = invocation.context().ok_or(Error::ContextRequired)?;
    let number = i64::try_from(context.operation_id)
        .map_err(|error| Error::JournalFailed(error.to_string()))?;
    let id = OperationId::new(number).map_err(|error| Error::JournalFailed(error.to_string()))?;
    let mut after = -1;
    let mut mixed = false;
    loop {
        let page = store
            .journal(id, after, PAGE_SIZE)
            .await
            .map_err(|error| Error::JournalFailed(error.to_string()))?;
        let Some(sequence) = inspect(&page, &mut mixed)? else {
            break;
        };
        after = sequence;
    }
    if mixed {
        return Err(Error::ChecksOrder.into());
    }
    Ok(())
}

fn inspect(page: &[(i64, Event)], mixed: &mut bool) -> Result<Option<i64>, Error> {
    for (_, event) in page {
        if checked(event) {
            return Err(Error::ChecksAlreadyPassed);
        }
        if let Event::Appended {
            message: Message::Assistant(response),
        } = event
        {
            *mixed = response
                .tool_calls
                .iter()
                .any(|call| call.name == "RunChecks");
        }
    }
    Ok(page.last().map(|(sequence, _)| *sequence))
}

fn checked(event: &Event) -> bool {
    match event {
        Event::Appended {
            message: Message::ToolResults(results),
        } => results
            .iter()
            .any(|result| result.name == "RunChecks" && !result.is_error),
        _ => false,
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
