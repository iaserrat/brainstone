use agent::Message;

/// How settle ended a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// The run finished normally.
    Finished,
    /// The run was corrected and continues with these messages.
    Steered(Vec<Message>),
}

pub(crate) async fn settle<S: crate::Store>(
    store: &S,
    lease: &crate::Lease,
    outcome: &agent::Outcome,
) -> Result<Settled, crate::Error> {
    store
        .record(
            lease,
            &agent::Event::RunFinished {
                outcome: outcome.clone(),
            },
        )
        .await?;
    Ok(Settled::Finished)
}
