use agent::{ModelResponse, Sink, Thread};
use serde_json::Value;

use crate::request::Request;
use crate::responses_model::ResponsesModel;

pub(crate) async fn generate<S: Sink + Send>(
    model: &ResponsesModel,
    thread: &Thread,
    tools: &[Value],
    sink: &mut S,
) -> Result<ModelResponse, S::Error> {
    let request = match Request::new(model, thread, tools) {
        Ok(request) => request.streaming(),
        Err(error) => return Ok(error.response()),
    };
    let response = crate::transport::execute(&model.http, &request, &model.parameters).await;
    let response = match response {
        Ok(response) => response,
        Err(error) => return Ok(error.response()),
    };
    match crate::stream::decode(response, sink).await {
        Ok(response) => Ok(response),
        Err(crate::stream::Error::Provider(error)) => Ok(error.response()),
        Err(crate::stream::Error::Sink(error)) => Err(error),
    }
}
