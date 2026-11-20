use brainstone_runtime::{LaneId, Operation, OperationId, OperationState, SessionId};
use serde_json::{Value, json};

fn operation(state: &str, outcome: &Value) -> Value {
    json!({"id": 1, "session_id": 2, "lane_id": 3, "state": state, "outcome": outcome})
}

#[test]
fn operation_identifiers_cannot_bypass_validation_through_deserialization() {
    for value in [i64::MIN, -1, 0] {
        assert!(serde_json::from_value::<OperationId>(json!(value)).is_err());
    }
    for value in [1, i64::MAX] {
        let id: OperationId = serde_json::from_value(json!(value)).unwrap();
        assert_eq!(id.get(), value);
        assert_eq!(serde_json::to_value(id).unwrap(), json!(value));
    }
}

#[test]
fn operation_deserialization_rejects_disagreeing_state_and_outcome() {
    let completed = json!({"Completed": {"text": "done"}});
    let failed = json!({"Failed": {"code": "example_failed", "message": "failed"}});
    for (state, outcome) in [
        ("accepted", completed.clone()),
        ("running", failed.clone()),
        ("completed", Value::Null),
        ("failed", Value::Null),
        ("completed", failed),
        ("failed", completed),
    ] {
        assert!(serde_json::from_value::<Operation>(operation(state, &outcome)).is_err());
    }
}

#[test]
fn valid_operations_keep_their_serialized_contract() {
    for (state, outcome) in [
        ("accepted", Value::Null),
        ("running", Value::Null),
        ("completed", json!({"Completed": {"text": "done: café"}})),
        (
            "failed",
            json!({"Failed": {"code": "example_failed", "message": "failed"}}),
        ),
    ] {
        let document = operation(state, &outcome);
        let decoded: Operation = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), document);
    }
}

#[test]
fn custom_stores_can_validate_operations_with_the_stable_corruption_code() {
    let mut decoded: Operation =
        serde_json::from_value(operation("accepted", &Value::Null)).unwrap();
    decoded.state = OperationState::Completed;
    assert_eq!(decoded.validate().unwrap_err().code(), "corrupt_record");
}

#[test]
fn custom_stores_construct_all_identifiers_without_deserialization() {
    let operation = Operation {
        id: OperationId::new(1).unwrap(),
        session_id: SessionId::new(2).unwrap(),
        lane_id: LaneId::new(3).unwrap(),
        state: OperationState::Accepted,
        outcome: None,
    };
    operation.validate().unwrap();
    assert_eq!(operation.session_id.get(), 2);
    assert_eq!(operation.lane_id.get(), 3);
}

#[test]
fn session_and_lane_identifiers_validate_construction_and_deserialization() {
    for value in [i64::MIN, -1, 0] {
        assert_eq!(SessionId::new(value).unwrap_err().code(), "invalid_input");
        assert_eq!(LaneId::new(value).unwrap_err().code(), "invalid_input");
        assert!(serde_json::from_value::<SessionId>(json!(value)).is_err());
        assert!(serde_json::from_value::<LaneId>(json!(value)).is_err());
    }
    for value in [1, i64::MAX] {
        let session = SessionId::try_from(value).unwrap();
        let lane = LaneId::try_from(value).unwrap();
        assert_eq!(serde_json::to_value(session).unwrap(), json!(value));
        assert_eq!(serde_json::to_value(lane).unwrap(), json!(value));
        assert_eq!(
            serde_json::from_value::<SessionId>(json!(value)).unwrap(),
            session
        );
        assert_eq!(
            serde_json::from_value::<LaneId>(json!(value)).unwrap(),
            lane
        );
    }
}
