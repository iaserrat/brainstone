use tokio::sync::watch;

pub(crate) struct Waiting<'a>(&'a watch::Sender<bool>);

impl<'a> Waiting<'a> {
    pub(crate) fn new(sender: &'a watch::Sender<bool>) -> Self {
        let _ = sender.send(true);
        Self(sender)
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let _ = self.0.send(false);
    }
}

pub(crate) fn no_call_key() -> rmcp::ErrorData {
    internal("elicitation is not associated with a durable tool call")
}

pub(crate) fn internal(error: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(error.to_string(), None)
}
