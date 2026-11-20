use crate::{ChatCompletionsModel, failure::Failure};
use agent::{ModelResponse, Thread};
use serde_json::Value;

pub(crate) async fn generate(
    model: &ChatCompletionsModel,
    thread: &Thread,
    tools: &[Value],
) -> Result<ModelResponse, Failure> {
    let request = crate::chat_request::request(model, thread, tools)?;
    let response = crate::transport::execute(&model.http, &request, &model.parameters).await?;
    crate::chat_response::parse(&crate::transport::decode(response).await?)
}
