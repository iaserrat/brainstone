use std::collections::HashMap;

use agent::{Event, Image, Message};

use crate::Error;
type HistoryRow = (
    i64,
    i64,
    Option<String>,
    Option<sqlx::types::Json<serde_json::Value>>,
    i16,
);

/// The lane's replayed attachments, keyed by owning operation.
pub(crate) struct Attachments<'a> {
    pub(crate) images: &'a HashMap<i64, Vec<Image>>,
}

pub(crate) fn reduce(
    rows: Vec<HistoryRow>,
    attachments: &Attachments<'_>,
    summary: Option<String>,
) -> Result<(Vec<Message>, usize), Error> {
    let mut state = State {
        messages: summary.into_iter().map(Message::Summary).collect(),
        steps: 0,
    };
    for (operation, _, prompt, payload, version) in rows {
        if let Some(prompt) = prompt {
            state.messages.push(user(prompt, operation, attachments));
            state.steps = 0;
            continue;
        }
        crate::stored_event::check_version(version)?;
        let value = payload.ok_or_else(crate::stored_event::missing_payload)?;
        let event: Event = serde_json::from_value(value.0).map_err(Error::corrupt)?;
        state.apply(event);
    }
    Ok((state.messages, state.steps))
}

fn user(text: String, operation: i64, attachments: &Attachments<'_>) -> Message {
    let images = attachments
        .images
        .get(&operation)
        .cloned()
        .unwrap_or_default();
    Message::user(text, images)
}

struct State {
    messages: Vec<Message>,
    steps: usize,
}

impl State {
    fn apply(&mut self, event: Event) {
        match event {
            Event::Appended { message } => self.messages.push(message),
            Event::Compacted { summary, steps } => {
                self.messages = vec![Message::Summary(summary)];
                self.steps = steps;
            }
            _ => {}
        }
    }
}
