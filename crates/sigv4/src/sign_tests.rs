use aws_credential_types::Credentials;
use aws_credential_types::provider::SharedCredentialsProvider;

use crate::{Error, SigV4};

fn signer() -> SigV4 {
    let credentials = Credentials::for_tests_with_session_token();
    SigV4::with_provider("ca-central-1", SharedCredentialsProvider::new(credentials))
}

#[tokio::test]
async fn a_signature_names_the_service_region_and_session() {
    let headers = signer()
        .headers(
            "bedrock",
            "https://bedrock-runtime.ca-central-1.amazonaws.com/openai/v1/responses",
            &[("content-type", "application/json")],
            b"{}",
        )
        .await
        .unwrap();

    let value = |name: &str| {
        headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.clone())
    };
    let authorization = value("authorization").expect("signed");
    assert!(
        authorization.contains("/ca-central-1/bedrock/aws4_request"),
        "{authorization}"
    );
    assert!(authorization.contains("content-type"), "{authorization}");
    assert!(value("x-amz-security-token").is_some());
    assert!(value("x-amz-date").is_some());
}

#[tokio::test]
async fn a_blank_region_is_rejected_before_any_request() {
    assert_eq!(SigV4::load(" ").await.unwrap_err(), Error::EmptyRegion);
    assert_eq!(Error::EmptyRegion.code(), "empty_region");
    assert_eq!(Error::MissingCredentials.code(), "missing_credentials");
}
