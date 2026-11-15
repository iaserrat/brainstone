use super::{BASE_DELAY, MAX_DELAY};
use reqwest::Response;
use std::time::Duration;
pub(super) fn delay(response: &Response, attempt: u32) -> Duration {
    retry_after(response)
        .unwrap_or(BASE_DELAY * 2_u32.pow(attempt))
        .min(MAX_DELAY)
}

fn retry_after(response: &Response) -> Option<Duration> {
    let milliseconds = response
        .headers()
        .get("retry-after-ms")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis);
    milliseconds.or_else(|| {
        response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .filter(|seconds: &f64| seconds.is_finite() && *seconds >= 0.0)
            .map(Duration::from_secs_f64)
    })
}
