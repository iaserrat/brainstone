use std::time::Duration;

use reqwest::{Response, Url};
use serde_json::Value;

use crate::failure::Failure;
use crate::model::AnthropicModel;

const VERSION: &str = "2023-06-01";
/// Sent on every request and covered by the `SigV4` signature when used.
const HEADERS: [(&str, &str); 2] = [
    ("content-type", "application/json"),
    ("anthropic-version", VERSION),
];
const MAX_RETRIES: u32 = 2;
const BASE_DELAY: Duration = Duration::from_millis(250);
const MAX_DELAY: Duration = Duration::from_mins(1);

pub(crate) async fn send(
    model: &AnthropicModel,
    url: &Url,
    body: &[u8],
) -> Result<Response, Failure> {
    for attempt in 0..=MAX_RETRIES {
        let span = agent::observation::span(agent::observation::Kind::Provider);
        span.record("attempt", attempt + 1);
        let response = agent::observation::run(span, request(model, url, body), |result| {
            attempt_outcome(result, attempt)
        })
        .await;
        match response {
            Err(_) if attempt < MAX_RETRIES => sleep(BASE_DELAY * 2_u32.pow(attempt)).await,
            Err(error) => return Err(error),
            Ok(response)
                if crate::http_status::retryable(response.status()) && attempt < MAX_RETRIES =>
            {
                sleep(delay(&response, attempt)).await;
            }
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => return Err(http_error(response).await),
        }
    }
    unreachable!("the bounded retry loop always returns")
}

async fn request(model: &AnthropicModel, url: &Url, body: &[u8]) -> Result<Response, Failure> {
    let auth = model.auth.headers(url, &HEADERS, body).await?;
    let mut request = model.client.post(url.clone());
    for (name, value) in HEADERS {
        request = request.header(name, value);
    }
    for (name, value) in auth {
        request = request.header(name, value);
    }
    if let Some(context) = telemetry::traceparent() {
        request = request.header("traceparent", context);
    }
    Ok(request.body(body.to_vec()).send().await?)
}

async fn http_error(response: Response) -> Failure {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| value["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or(body);
    Failure::new(crate::http_status::code(status), message)
}

fn delay(response: &Response, attempt: u32) -> Duration {
    response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map_or(BASE_DELAY * 2_u32.pow(attempt), Duration::from_secs_f64)
        .min(MAX_DELAY)
}

async fn sleep(delay: Duration) {
    tokio::time::sleep(delay).await;
}

fn attempt_outcome(result: &Result<Response, Failure>, attempt: u32) -> &'static str {
    match result {
        Ok(response) if response.status().is_success() => "ok",
        Ok(response)
            if attempt < MAX_RETRIES && crate::http_status::retryable(response.status()) =>
        {
            "retry"
        }
        Err(_) if attempt < MAX_RETRIES => "retry",
        _ => "failed",
    }
}
