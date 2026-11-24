//! Contract: a model's published capacity is read by exact id, and the models
//! endpoint is derived only from the chat-completions shape.

use reqwest::Url;
use serde_json::json;

#[test]
fn the_declared_capacity_of_the_named_model() {
    let document = json!({"data": [
        {"id": "other", "context_length": 32_768},
        {"id": "wanted", "context_length": 1_048_576},
    ]});

    assert_eq!(super::context_length(&document, "wanted"), Some(1_048_576));
    assert_eq!(super::context_length(&document, "missing"), None);
}

#[test]
fn a_document_without_a_usable_capacity_yields_nothing() {
    for document in [
        json!({}),
        json!({"data": []}),
        json!({"data": [{"id": "wanted"}]}),
        json!({"data": [{"id": "wanted", "context_length": 0}]}),
        json!({"data": [{"id": "wanted", "context_length": -1}]}),
        json!({"data": "wanted"}),
    ] {
        assert_eq!(super::context_length(&document, "wanted"), None);
    }
}

#[test]
fn the_models_endpoint_is_the_chat_endpoints_sibling() {
    let url = Url::parse("https://router.example/api/v1/chat/completions").unwrap();
    assert_eq!(
        super::models_url(&url).map(|url| url.to_string()),
        Some("https://router.example/api/v1/models".to_owned())
    );
}

#[test]
fn an_endpoint_that_is_not_chat_completions_yields_nothing() {
    for endpoint in [
        "https://router.example/api/v1/responses",
        "https://router.example/mcp",
        "https://router.example/chat/completions/extra",
    ] {
        let url = Url::parse(endpoint).unwrap();
        assert_eq!(super::models_url(&url), None, "{endpoint}");
    }
}
