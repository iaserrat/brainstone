//! Authoritative operation evidence comes from its entire journal, not replay context.
use agent::{Event, Message, Thread};
use serde_json::Value;
use sqlx::types::Json;

use crate::Error;

pub(crate) type EvidenceRow = (i16, Json<Value>);

#[derive(Default)]
pub(crate) struct Evidence {
    responses: usize,
    successes: Vec<String>,
}

impl Evidence {
    pub(crate) fn responses(count: i64) -> Result<Self, Error> {
        Ok(Self {
            responses: usize::try_from(count).map_err(Error::corrupt)?,
            successes: Vec::new(),
        })
    }

    pub(crate) fn decode(rows: Vec<EvidenceRow>) -> Result<Self, Error> {
        let mut evidence = Self::default();
        for (version, payload) in rows {
            crate::stored_event::check_version(version)?;
            let event = serde_json::from_value(payload.0).map_err(Error::corrupt)?;
            evidence.observe(&event);
        }
        Ok(evidence)
    }

    pub(crate) fn observe(&mut self, event: &Event) {
        match event {
            Event::Appended {
                message: Message::Assistant(_),
            } => self.responses += 1,
            Event::Appended {
                message: Message::ToolResults(results),
            } => self.successes.extend(
                results
                    .iter()
                    .filter(|result| !result.is_error)
                    .map(|result| result.name.clone()),
            ),
            _ => {}
        }
    }

    pub(crate) fn restore(self, thread: Thread) -> Thread {
        thread
            .with_tool_sequence(self.responses)
            .with_tool_successes(self.successes)
    }
}
