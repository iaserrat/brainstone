use super::{Message, Thread};

impl Thread {
    /// Replaces the model-visible prefix with `summary`, keeping replay
    /// consistent with what the provider will see.
    pub fn compact(&mut self, summary: String) {
        self.prior_steps = self.steps();
        self.messages = vec![Message::Summary(summary)];
    }

    /// Model responses so far. Derived rather than counted, so a restored
    /// thread reports the same budget a live one does.
    #[must_use]
    pub fn steps(&self) -> usize {
        let (steps, user_turn) = self.tail_steps();
        if user_turn {
            steps
        } else {
            self.prior_steps + steps
        }
    }

    /// The historical step component of a durable tool key. Keep the compacted
    /// count after steering: older writers already issued keys with that count.
    #[must_use]
    pub fn tool_step(&self) -> usize {
        self.prior_steps + self.tail_steps().0
    }

    fn tail_steps(&self) -> (usize, bool) {
        let mut steps = 0;
        for message in self.messages.iter().rev() {
            match message {
                message if message.is_user_turn() => return (steps, true),
                Message::Assistant(_) => steps += 1,
                _ => {}
            }
        }
        (steps, false)
    }
}
