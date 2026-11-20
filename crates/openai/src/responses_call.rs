use agent::{Model, ModelResponse, Sink, Thread};
use serde_json::Value;

use crate::failure::{self, CODE_INVALID_RESPONSE};
use crate::request::Request;
use crate::response::Response;
use crate::responses_model::ResponsesModel;

impl Model for ResponsesModel {
    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        generate(self, thread, tools)
            .await
            .unwrap_or_else(failure::Failure::response)
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        crate::responses_stream::generate(self, thread, tools, sink).await
    }
}

async fn generate(
    model: &ResponsesModel,
    thread: &Thread,
    tools: &[Value],
) -> Result<ModelResponse, failure::Failure> {
    let request = Request::new(model, thread, tools)?;
    let response = crate::transport::execute(&model.http, &request, &model.parameters).await?;
    serde_json::from_value::<Response>(crate::transport::decode(response).await?)
        .map_err(|error| failure::Failure::new(CODE_INVALID_RESPONSE, error.to_string()))?
        .into_model()
}
