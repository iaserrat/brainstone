use serde_json::Value;

use crate::{DeliveryTarget, Error, OperationId};

/// A question a surface must deliver and answer.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionRequest {
    /// The operation waiting for the answer.
    pub operation_id: OperationId,
    /// The store's id for the interaction.
    pub id: String,
    /// The question, as the surface renders it.
    pub body: Value,
}

/// A delivered question and its answer, if one exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Interaction {
    /// The question.
    pub request: InteractionRequest,
    /// Where the question was posted.
    pub target: DeliveryTarget,
    /// The id a retry reuses so the post is not duplicated.
    pub client_message_id: String,
    /// The surface's id for the posted message.
    pub external_id: Option<String>,
    /// The answer, once the surface supplied one.
    pub response: Option<Value>,
}

impl InteractionRequest {
    /// # Errors
    ///
    /// Returns `invalid_input` when the durable request ID is empty.
    pub fn new(
        operation_id: OperationId,
        id: impl Into<String>,
        body: Value,
    ) -> Result<Self, Error> {
        let id = id.into();
        if id.is_empty() {
            return Err(Error::InvalidInput {
                field: "interaction_id",
            });
        }
        Ok(Self {
            operation_id,
            id,
            body,
        })
    }
}

/// Durable interactions: open, deliver, answer, and read.
pub trait InteractionStore: Clone + Send + Sync {
    /// Opens a question, or returns the one the same id already opened.
    fn open_interaction(
        &self,
        request: &InteractionRequest,
    ) -> impl Future<Output = Result<Interaction, Error>> + Send;

    /// Records the surface's id for the posted question.
    fn mark_interaction_delivered(
        &self,
        id: &str,
        external_id: &str,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Stores the answer.
    fn answer_interaction(
        &self,
        id: &str,
        response: &Value,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// The interaction, if it exists.
    fn interaction(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<Interaction>, Error>> + Send;
}
