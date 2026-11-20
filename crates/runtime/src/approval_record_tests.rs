use super::{Row, decode};
use crate::{ApprovalRequest, ApprovalState};
use serde_json::json;
fn row(version: u8) -> Row {
    let request: ApprovalRequest = serde_json::from_value(json!({
        "operation":1,"call_key":"operation/1/step/0/call/one",
        "call":{"id":"one","name":"External","args":{}},
        "definition":{},"identity":null,"binding":"policy","version":version
    }))
    .unwrap();
    (
        request.id(),
        serde_json::to_string(&request).unwrap(),
        None,
        None,
        100,
        0,
    )
}
#[test]
fn unknown_versions_and_decisions_are_corruption_even_after_expiry() {
    assert_eq!(decode(row(2)).unwrap_err().code(), "corrupt_record");
    let mut value = row(1);
    value.2 = Some("unknown".into());
    value.5 = 200;
    assert_eq!(decode(value).unwrap_err().code(), "corrupt_record");
}
#[test]
fn digest_mismatches_fail_closed_and_expired_requests_cannot_authorize() {
    let mut value = row(1);
    value.0 = "invalid".into();
    assert_eq!(decode(value).unwrap_err().code(), "corrupt_record");
    let mut value = row(1);
    value.2 = Some("approved".into());
    value.5 = 100;
    assert_eq!(decode(value).unwrap().state, ApprovalState::Expired);
}
