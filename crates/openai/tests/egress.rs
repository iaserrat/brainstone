//! Where a client connects is a policy, so it is asserted at the socket.
use agent::{Egress, Model, Thread};
use brainstone_openai::ChatCompletionsModel;
use serde_json::json;
use std::time::Duration;

#[path = "support/http.rs"]
mod http;
use http::serve_once;

#[tokio::test]
async fn an_egress_proxy_receives_the_request_in_absolute_form_for_the_real_host() {
    let answer = json!({"choices":[{"message":{"role":"assistant","content":"proxied"},
        "finish_reason":"stop"}]});
    let (proxied, captured) = serve_once(answer).await;
    let proxy = proxied.trim_end_matches("/v1/responses").to_owned();
    let model = ChatCompletionsModel::unauthenticated(
        "http://provider.invalid/v1/chat/completions",
        "local",
    )
    .unwrap()
    .with_idle_timeout(Duration::from_secs(5))
    .unwrap()
    .with_egress(&Egress::proxy(&proxy))
    .unwrap();
    let response = model
        .generate(&Thread::new("system", "where do you connect?"), &[])
        .await;
    assert_eq!(response.text, "proxied");
    let captured = captured.await.unwrap();
    assert_eq!(
        captured.line,
        "POST http://provider.invalid/v1/chat/completions HTTP/1.1"
    );
}

#[tokio::test]
async fn a_proxy_scheme_the_client_cannot_speak_is_refused_by_code() {
    let refused = ChatCompletionsModel::unauthenticated("http://provider.invalid/v1/chat", "local")
        .unwrap()
        .with_egress(&Egress::proxy("socks5://127.0.0.1:9050"));
    assert_eq!(refused.err().unwrap().code(), "invalid_proxy");
}
