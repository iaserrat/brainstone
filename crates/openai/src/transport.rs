use std::time::Duration;
#[path = "transport_delay.rs"]
mod backoff;
use backoff::delay;

use reqwest::{Response, StatusCode};
use serde::Serialize;
use serde_json::Value;

use crate::failure::{CODE_INVALID_RESPONSE, Failure};
use crate::http::Http;

const MAX_RETRIES: u32 = 3;
const BASE_DELAY: Duration = Duration::from_millis(250);
const MAX_DELAY: Duration = Duration::from_mins(1);

/// Sent on every request and covered by the `SigV4` signature when used.
const HEADERS: [(&str, &str); 1] = [("content-type", "application/json")];

pub(crate) async fn execute<T: Serialize + ?Sized>(
    model: &Http,
    body: &T,
    parameters: &crate::RequestParameters,
) -> Result<Response, Failure> {
    let body = parameters.encode(body)?;
    for attempt in 0..=MAX_RETRIES {
        let span = agent::observation::span(agent::observation::Kind::Provider);
        span.record("attempt", attempt + 1);
        let response = agent::observation::run(span, send(model, &body), |result| {
            attempt_outcome(result, attempt)
        })
        .await?;
        if crate::transport_status::retryable(response.status()) && attempt < MAX_RETRIES {
            tokio::time::sleep(delay(&response, attempt)).await;
            continue;
        }
        return Ok(response);
    }
    unreachable!("the bounded retry loop always returns")
}

/// Shared by both protocols; `SigV4` signs each attempt's URL, required headers, and body.
async fn send(model: &Http, body: &[u8]) -> Result<Response, Failure> {
    let mut request = model
        .client
        .post(model.endpoint.clone())
        .headers(model.headers.clone());
    for (name, value) in HEADERS {
        request = request.header(name, value);
    }
    let url = model.endpoint.as_str();
    let mut request = model.signer.authorize(request, url, &HEADERS, body).await?;
    if let Some(context) = telemetry::traceparent() {
        request = request.header("traceparent", context);
    }
    Ok(request.body(body.to_vec()).send().await?)
}

pub(crate) async fn decode(response: Response) -> Result<Value, Failure> {
    let response = require_success(response).await?;
    let body = response.text().await?;
    serde_json::from_str(&body)
        .map_err(|error| Failure::new(CODE_INVALID_RESPONSE, error.to_string()))
}

pub(crate) async fn require_success(response: Response) -> Result<Response, Failure> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await?;
    Err(rejection(status, body))
}

fn rejection(status: StatusCode, body: String) -> Failure {
    let error = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|mut value| value.get_mut("error").map(Value::take))
        .unwrap_or_default();
    let code = crate::failure::classified(
        error["code"].as_str(),
        crate::transport_status::code(status),
    );
    let message = error["message"].as_str().map_or(body, str::to_owned);
    Failure::new(code, message)
}

fn attempt_outcome(result: &Result<Response, Failure>, attempt: u32) -> &'static str {
    match result {
        Ok(response) if response.status().is_success() => "ok",
        Ok(response)
            if attempt < MAX_RETRIES && crate::transport_status::retryable(response.status()) =>
        {
            "retry"
        }
        _ => "failed",
    }
}
