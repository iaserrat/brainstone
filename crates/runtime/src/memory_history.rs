use std::collections::HashMap;

use agent::{Event, Message, Thread};

use crate::{Compaction, Operation, OperationId, Prompt};

pub(crate) struct History<'a> {
    pub(crate) operations: &'a HashMap<OperationId, Operation>,
    pub(crate) prompts: &'a HashMap<OperationId, Prompt>,
    pub(crate) events: &'a HashMap<OperationId, Vec<Event>>,
    pub(crate) compaction: Option<&'a Compaction>,
}

pub(crate) fn load(history: &History<'_>, current: OperationId) -> Thread {
    let lane_id = history.operations[&current].lane_id;
    let cutoff = history.compaction.map_or(0, |item| item.through.0);
    let mut ids: Vec<OperationId> = history
        .operations
        .values()
        .filter(|operation| {
            operation.lane_id == lane_id && operation.id.0 > cutoff && operation.id.0 <= current.0
        })
        .map(|operation| operation.id)
        .collect();
    ids.sort_by_key(|id| id.0);
    let system = &history.prompts[&current].system;
    let (messages, steps) = messages(&ids, history);
    let profile = history.prompts[&current].profile.clone();
    Thread::restore_compacted(system, messages, profile, steps)
        .expect("an operation has its prompt")
}

fn messages(ids: &[OperationId], history: &History<'_>) -> (Vec<Message>, usize) {
    let mut messages = Vec::new();
    let mut steps = 0;
    if let Some(compaction) = history.compaction {
        messages.push(Message::Summary(compaction.summary.clone()));
    }
    for id in ids {
        messages.push(history.prompts[id].message());
        steps = 0;
        for event in history.events.get(id).into_iter().flatten() {
            match event {
                Event::Appended { message } => messages.push(message.clone()),
                Event::Compacted {
                    summary,
                    steps: prior,
                } => {
                    messages = vec![Message::Summary(summary.clone())];
                    steps = *prior;
                }
                _ => {}
            }
        }
    }
    (messages, steps)
}
