//! Accepted tool contracts validate capability access and committed success evidence.
use brainstone_agent::{
    ExecutionProfile, Message, ModelResponse, Outcome, Step, StopReason, Thread, ToolContract,
    ToolResult,
};
use serde_json::json;

fn profile() -> ExecutionProfile {
    ExecutionProfile::default().with_tool_contract(
        ToolContract::only(["Refund"])
            .unwrap()
            .require_approval(["Refund"])
            .unwrap()
            .require_success(["Refund"])
            .unwrap(),
    )
}
fn answer(thread: &mut Thread) {
    thread.push(Message::Assistant(ModelResponse {
        reasoning: vec![],
        text: "done".into(),
        tool_calls: vec![],
        stop: StopReason::EndTurn,
        model: None,
    }));
}
fn success(thread: &mut Thread, name: &str, is_error: bool) {
    thread.push(Message::ToolResults(vec![ToolResult {
        id: "call".into(),
        name: name.into(),
        content: "{}".into(),
        is_error,
        images: vec![],
    }]));
}
fn failure(thread: &Thread) -> String {
    let Step::Done(Outcome::Failed { code, .. }) = brainstone_agent::next(thread) else {
        panic!("expected failure")
    };
    code
}
#[test]
fn missing_or_failed_effect_cannot_complete() {
    let mut thread = Thread::new_with_profile("s", "p", profile());
    success(&mut thread, "Refund", true);
    answer(&mut thread);
    assert_eq!(failure(&thread), "tool_requirement_unmet");
}
#[test]
fn committed_effect_survives_compaction_and_round_trip() {
    let mut thread = Thread::new_with_profile("s", "p", profile());
    success(&mut thread, "Refund", false);
    thread.compact("summary".into());
    answer(&mut thread);
    let thread: Thread = serde_json::from_value(serde_json::to_value(thread).unwrap()).unwrap();
    assert!(matches!(
        brainstone_agent::next(&thread),
        Step::Done(Outcome::Completed { .. })
    ));
}
#[test]
fn restored_history_is_not_authoritative_current_operation_evidence() {
    let mut old = Thread::new_with_profile("s", "p", profile());
    success(&mut old, "Refund", false);
    let mut restored =
        Thread::restore_with_profile("s", old.messages().to_vec(), profile()).unwrap();
    answer(&mut restored);
    assert_eq!(failure(&restored), "tool_evidence_unavailable");
    let restored = restored.with_tool_successes(vec![]);
    assert_eq!(failure(&restored), "tool_requirement_unmet");
    let restored = restored.with_tool_successes(vec!["Refund".into()]);
    assert!(matches!(
        brainstone_agent::next(&restored),
        Step::Done(Outcome::Completed { .. })
    ));
}
#[test]
fn contract_sets_replace_and_reject_invalid_relationships() {
    let contract = ToolContract::only(["Refund", "Read", "Refund"])
        .unwrap()
        .require_success(["Read"])
        .unwrap()
        .require_success(["Refund"])
        .unwrap();
    assert_eq!(contract.required_tools(), &["Refund"]);
    assert!(contract.allows("Read"));
    assert!(!contract.allows("Other"));
    assert!(contract.require_approval(["Other"]).is_err());
    assert!(ToolContract::only([""]).is_err());
    assert!(ToolContract::only(["*"]).is_err());
    assert!(ToolContract::default().allows("Anything"));
}
#[test]
fn serde_is_strict_and_profiles_pin_contracts() {
    for invalid in [
        json!({"allow":["Read"],"require_success":["Refund"]}),
        json!({"allow":["Read"],"future":true}),
        json!({"allow":[""]}),
    ] {
        assert!(serde_json::from_value::<ToolContract>(invalid).is_err());
    }
    let profile = profile();
    let restored: ExecutionProfile =
        serde_json::from_value(serde_json::to_value(&profile).unwrap()).unwrap();
    assert!(profile.same_settings(&restored));
    assert!(!profile.same_settings(&ExecutionProfile::default()));
    assert!(
        restored
            .tool_contract()
            .unwrap()
            .approval_required("Refund")
    );
}

#[test]
fn names_have_exact_boundaries_and_case_sensitive_matching() {
    for name in ["", "Zoë", "a b", "*", &"a".repeat(65)] {
        assert_eq!(
            ToolContract::only([name]).unwrap_err().code(),
            "invalid_tool_contract"
        );
    }
    let longest = "a".repeat(64);
    assert!(
        ToolContract::only([longest.as_str()])
            .unwrap()
            .allows(&longest)
    );
    assert!(!ToolContract::only(["Refund"]).unwrap().allows("refund"));
    assert!(
        !ToolContract::only(Vec::<String>::new())
            .unwrap()
            .allows("Refund")
    );
}

#[test]
fn legacy_profiles_remain_unrestricted_and_failures_keep_their_code() {
    let legacy: ExecutionProfile = serde_json::from_value(
        json!({"model":null,"effort":null,"max_turns":"unlimited","depth":0}),
    )
    .unwrap();
    assert!(legacy.tool_contract().is_none());
    let mut thread = Thread::new_with_profile("s", "p", legacy);
    answer(&mut thread);
    assert!(matches!(
        brainstone_agent::next(&thread),
        Step::Done(Outcome::Completed { .. })
    ));
    let mut thread = Thread::new_with_profile("s", "p", profile());
    thread.push(Message::Assistant(ModelResponse {
        reasoning: vec![],
        text: String::new(),
        tool_calls: vec![],
        stop: StopReason::Error {
            code: "provider_failure".into(),
            message: "unavailable".into(),
        },
        model: None,
    }));
    assert_eq!(failure(&thread), "provider_failure");
}
