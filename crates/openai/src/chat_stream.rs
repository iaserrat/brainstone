use crate::{
    ChatCompletionsModel,
    failure::{CODE_PROVIDER_FAILED, Failure},
    stream::Error,
};
use agent::{ModelResponse, Sink, Thread};
use serde_json::Value;

pub(crate) async fn generate<S: Sink + Send>(
    model: &ChatCompletionsModel,
    thread: &Thread,
    tools: &[Value],
    sink: &mut S,
) -> Result<ModelResponse, S::Error> {
    match start(model, thread, tools, sink).await {
        Ok(response) => Ok(response),
        Err(Error::Provider(error)) => Ok(error.response()),
        Err(Error::Sink(error)) => Err(error),
    }
}

async fn start<S: Sink + Send>(
    model: &ChatCompletionsModel,
    thread: &Thread,
    tools: &[Value],
    sink: &mut S,
) -> Result<ModelResponse, Error<S::Error>> {
    let mut request =
        crate::chat_request::request(model, thread, tools).map_err(Error::Provider)?;
    request["stream"] = Value::Bool(true);
    let response = crate::transport::execute(&model.http, &request, &model.parameters)
        .await
        .map_err(Error::Provider)?;
    let response = crate::transport::require_success(response)
        .await
        .map_err(Error::Provider)?;
    crate::chat_stream_state::State::default()
        .run(response, sink)
        .await
}

pub(crate) fn rejection(value: &Value) -> Failure {
    let error = &value["error"];
    let status = error["code"]
        .as_u64()
        .and_then(|code| u16::try_from(code).ok())
        .and_then(|code| reqwest::StatusCode::from_u16(code).ok());
    let fallback = status.map_or(CODE_PROVIDER_FAILED, crate::transport_status::code);
    let code = crate::failure::classified(error["code"].as_str(), fallback);
    Failure::new(
        code,
        error["message"]
            .as_str()
            .unwrap_or("provider stream failed"),
    )
}
