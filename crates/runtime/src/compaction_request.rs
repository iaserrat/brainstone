//! The requests a summary is asked from, most faithful first. Each rung after
//! the first answers a provider that refused the one before as too long.

use agent::{Message, Thread};

use super::replay;

/// Whether binaries replay, and how many of the newest turns stay.
pub(super) struct Rung {
    binaries: bool,
    turns: usize,
}

/// The whole history first, with its binaries when the replay bound allows
/// them; then the whole history as text; then the newest half of the turns,
/// halving until one turn is left.
pub(super) fn ladder(thread: &Thread, binaries: bool) -> Vec<Rung> {
    let mut turns = starts(thread).len();
    let mut rungs = Vec::new();
    if binaries && replay::replay_attachments(thread).0 > 0 {
        rungs.push(Rung { binaries, turns });
    }
    loop {
        rungs.push(Rung {
            binaries: false,
            turns,
        });
        if turns == 1 {
            return rungs;
        }
        turns = turns.div_ceil(2);
    }
}

/// The rung's messages, every tool result held to the bound, and `prompt`.
pub(super) fn build(thread: &Thread, rung: &Rung, prompt: &str) -> Thread {
    let starts = starts(thread);
    let first = starts[starts.len() - rung.turns];
    let messages = thread.messages()[first..]
        .iter()
        .map(|message| bounded(message, rung.binaries))
        .chain(std::iter::once(Message::User(prompt.to_owned())))
        .collect();
    Thread::restore_with_profile(&thread.system, messages, thread.profile().clone())
        .expect("a request carries its prompt")
}

/// Where each turn begins: the head, and every user turn after it. A request
/// may only start there, because a provider rejects one that opens on an
/// assistant message or on tool results whose calls it cannot see.
fn starts(thread: &Thread) -> Vec<usize> {
    thread
        .messages()
        .iter()
        .enumerate()
        .filter(|(index, message)| *index == 0 || message.is_user_turn())
        .map(|(index, _)| index)
        .collect()
}

fn bounded(message: &Message, binaries: bool) -> Message {
    let message = if binaries {
        message.clone()
    } else {
        replay::without_binaries(message)
    };
    match message {
        Message::ToolResults(results) => Message::ToolResults(
            results
                .into_iter()
                .map(agent::ToolResult::bounded)
                .collect(),
        ),
        other => other,
    }
}
