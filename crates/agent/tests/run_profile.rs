use brainstone_agent::{
    Effort, ExecutionProfile, Message, ModelResponse, Outcome, Step, StopReason, Thread, ToolCall,
    ToolResult, next,
};

#[test]
fn ordinary_runs_continue_past_one_hundred_model_responses() {
    let mut thread = Thread::new("system", "task");
    for index in 0..=brainstone_agent::MAX_STEPS {
        let id = format!("call-{index}");
        thread.push(Message::Assistant(ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: id.clone(),
                name: "Read".to_owned(),
                args: serde_json::json!({}),
            }],
            stop: StopReason::ToolUse,
        }));
        thread.push(answered(&id));
    }
    assert_eq!(next(&thread), Step::CallModel);
}

#[test]
fn execution_profile_bounds_model_turns() {
    let profile = ExecutionProfile::new(Some("claude-sonnet-5"), Some(Effort::High), 1, 2)
        .expect("valid profile");
    let mut thread = Thread::new_with_profile("system", "task", profile);
    thread.push(Message::Assistant(ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-1".to_owned(),
            name: "Read".to_owned(),
            args: serde_json::json!({}),
        }],
        stop: StopReason::ToolUse,
    }));
    thread.push(answered("call-1"));

    assert!(
        matches!(next(&thread), Step::Done(Outcome::Failed { code, .. }) if code == "step_budget_exhausted")
    );
    assert_eq!(thread.profile().model(), Some("claude-sonnet-5"));
    assert_eq!(thread.profile().effort(), Some(Effort::High));
    assert_eq!(thread.profile().depth(), 2);
}

#[test]
fn turn_limits_round_trip_without_changing_legacy_counts() {
    for (limit, wire) in [
        (None, serde_json::json!("unlimited")),
        (Some(7), serde_json::json!(7)),
    ] {
        let profile = ExecutionProfile::new(None, None, limit, 0).unwrap();
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(json["max_turns"], wire);
        let restored: ExecutionProfile = serde_json::from_value(json).unwrap();
        assert_eq!(restored.max_turns(), limit);
    }
    let legacy: ExecutionProfile = serde_json::from_value(serde_json::json!({
        "model": null, "effort": null, "max_turns": 25, "depth": 0
    }))
    .unwrap();
    assert_eq!(legacy.max_turns(), Some(25));
}

#[test]
fn missing_and_invalid_stored_limits_never_become_unlimited() {
    for invalid in [
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(null),
        serde_json::json!("100"),
        serde_json::json!(true),
        serde_json::json!("unknown"),
        serde_json::json!({}),
        serde_json::json!([]),
    ] {
        let json = serde_json::json!({"model":null,"effort":null,"max_turns":invalid,"depth":0});
        assert!(serde_json::from_value::<ExecutionProfile>(json).is_err());
    }
    let missing = serde_json::json!({"model":null,"effort":null,"depth":0});
    assert!(serde_json::from_value::<ExecutionProfile>(missing).is_err());
    assert_eq!(
        ExecutionProfile::new(None, None, Some(0), 0)
            .unwrap_err()
            .code(),
        "invalid_max_turns"
    );
}

#[test]
fn compaction_and_restore_keep_unlimited_execution() {
    let mut thread = Thread::restore_compacted(
        "system",
        vec![Message::Summary("saved".into())],
        ExecutionProfile::default(),
        brainstone_agent::MAX_STEPS + 1,
    )
    .unwrap();
    assert_eq!(next(&thread), Step::CallModel);
    thread.compact("compacted again".into());
    let restored: Thread = serde_json::from_value(serde_json::to_value(&thread).unwrap()).unwrap();
    assert_eq!(restored.profile().max_turns(), None);
    assert_eq!(next(&restored), Step::CallModel);
}

#[test]
fn legacy_threads_without_a_profile_keep_their_original_limit() {
    let mut json = serde_json::to_value(Thread::new("system", "work")).unwrap();
    json.as_object_mut().unwrap().remove("profile");
    let restored: Thread = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.profile().max_turns(),
        Some(brainstone_agent::MAX_STEPS)
    );
}

fn answered(id: &str) -> Message {
    Message::ToolResults(vec![ToolResult {
        id: id.to_owned(),
        name: "Read".to_owned(),
        content: "done".to_owned(),
        is_error: false,
        images: Vec::new(),
    }])
}
