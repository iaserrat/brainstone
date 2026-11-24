//! What the router publishes about a model's prompt capacity.
//!
//! A declared window lets the runtime compact below what the model actually
//! holds. Asking the router once at startup beats a hard-coded table that goes
//! stale, and any failure here is silent: the caller keeps its own budget.

use reqwest::Url;
use serde_json::Value;

/// Ask the router what capacity the model declares.
///
/// Every failure is `None`: an unreachable or silent catalog means the caller
/// keeps the budget it configured, and the model call reports the real problem.
pub(crate) async fn discover(http: &crate::http::Http, model: &str) -> Option<usize> {
    let url = models_url(&http.endpoint)?;
    let response = http
        .client
        .get(url)
        .headers(http.headers.clone())
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let document: Value = response.json().await.ok()?;
    context_length(&document, model)
}

/// The capacity this model declares in a `/models` document.
#[must_use]
pub(crate) fn context_length(document: &Value, model: &str) -> Option<usize> {
    document
        .get("data")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(model))?
        .get("context_length")?
        .as_u64()
        .and_then(|tokens| usize::try_from(tokens).ok())
        .filter(|tokens| *tokens > 0)
}

/// The sibling of a chat-completions endpoint that lists models.
///
/// `…/v1/chat/completions` becomes `…/v1/models`; any other shape yields
/// nothing, because guessing a URL is how a startup probe turns into a request
/// to somewhere unintended.
#[must_use]
pub(crate) fn models_url(endpoint: &Url) -> Option<Url> {
    let mut path = endpoint.path_segments()?.collect::<Vec<_>>();
    if path.pop()? != "completions" {
        return None;
    }
    if path.pop()? != "chat" {
        return None;
    }
    path.push("models");
    let mut url = endpoint.clone();
    url.set_path(&path.join("/"));
    url.set_query(None);
    Some(url)
}

#[cfg(test)]
#[path = "model_catalog_tests.rs"]
mod tests;
