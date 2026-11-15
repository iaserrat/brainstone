//! Behaviour of the decision function, one case per transition it can take.
//! The point of this file is the table, so it is exempt from the line budget.
#![allow(clippy::too_many_lines)]

use serde_json::json;

use crate::model::{ModelResponse, StopReason, ToolCall};
use crate::next::next;
use crate::step::{CODE_STEP_BUDGET, MAX_STEPS, Outcome, SILENCE_TOOL, Step};
use crate::thread::{Message, Thread};
use crate::tool::ToolResult;

fn call(id: &str) -> ToolCall {
    ToolCall {
        id: id.to_owned(),
        name: "search".to_owned(),
        args: json!({ "q": "café ☕" }),
    }
}

fn assistant(stop: StopReason, text: &str, tool_calls: Vec<ToolCall>) -> Message {
    Message::Assistant(ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls,
        stop,
    })
}

fn results(id: &str) -> Message {
    Message::ToolResults(vec![result(id, "search", false)])
}

#[test]
fn a_fresh_prompt_calls_the_model() {
    let thread = Thread::new("be useful", "hello");
    assert_eq!(next(&thread), Step::CallModel);
}

#[test]
fn answered_tool_calls_return_to_the_model() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::ToolUse, "", vec![call("c1")]));
    thread.push(results("c1"));
    assert_eq!(next(&thread), Step::CallModel);
}

#[test]
fn a_plain_answer_completes_the_run() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::EndTurn, "done ✅", Vec::new()));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: "done ✅".to_owned()
        })
    );
}

#[test]
fn requested_tools_run() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(
        StopReason::ToolUse,
        "",
        vec![call("c1"), call("c2")],
    ));
    assert_eq!(next(&thread), Step::RunTools(vec![call("c1"), call("c2")]));
}

/// A provider that reports `ToolUse` and asks for nothing would otherwise hang
/// the loop. Deliver what it said instead.
#[test]
fn tool_use_without_calls_completes_rather_than_hanging() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::ToolUse, "oops", Vec::new()));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: "oops".to_owned()
        })
    );
}

/// Truncated prose is still worth delivering; the run simply ends there.
#[test]
fn a_truncated_answer_without_calls_completes() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::MaxTokens, "half a sen", Vec::new()));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: "half a sen".to_owned()
        })
    );
}

/// The case that matters: streamed arguments are salvage-parsed, so a call cut
/// off by the token limit can still parse and validate while being incomplete.
/// None of the batch is safe to run.
#[test]
fn a_truncated_batch_is_failed_rather_than_executed() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(
        StopReason::MaxTokens,
        "",
        vec![call("c1"), call("c2")],
    ));
    assert_eq!(
        next(&thread),
        Step::FailTruncated(vec![call("c1"), call("c2")])
    );
}

#[test]
fn a_provider_error_fails_the_run_with_its_code() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(
        StopReason::Error {
            code: "rate_limited".to_owned(),
            message: "slow down".to_owned(),
        },
        "",
        Vec::new(),
    ));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Failed {
            code: "rate_limited".to_owned(),
            message: "slow down".to_owned(),
        })
    );
}

#[test]
fn the_step_budget_stops_a_loop_that_never_converges() {
    let profile = crate::ExecutionProfile::new(None, None, MAX_STEPS, 0).unwrap();
    let mut thread = Thread::new_with_profile("s", "hello", profile);
    for i in 0..MAX_STEPS {
        let id = format!("c{i}");
        thread.push(assistant(StopReason::ToolUse, "", vec![call(&id)]));
        thread.push(results(&id));
    }
    assert_eq!(thread.steps(), MAX_STEPS);
    let Step::Done(Outcome::Failed { code, .. }) = next(&thread) else {
        panic!("expected the budget to stop the run");
    };
    assert_eq!(code, CODE_STEP_BUDGET);
}

#[test]
fn one_step_below_the_budget_still_calls_the_model() {
    let profile = crate::ExecutionProfile::new(None, None, MAX_STEPS, 0).unwrap();
    let mut thread = Thread::new_with_profile("s", "hello", profile);
    for i in 0..MAX_STEPS - 1 {
        let id = format!("c{i}");
        thread.push(assistant(StopReason::ToolUse, "", vec![call(&id)]));
        thread.push(results(&id));
    }
    assert_eq!(next(&thread), Step::CallModel);
}

/// Tools the model already asked for still run at the budget, so the thread is
/// left well-formed. Providers reject a batch that was never answered.
#[test]
fn pending_tools_still_run_at_the_budget() {
    let profile = crate::ExecutionProfile::new(None, None, MAX_STEPS, 0).unwrap();
    let mut thread = Thread::new_with_profile("s", "hello", profile);
    for i in 0..MAX_STEPS {
        let id = format!("c{i}");
        thread.push(assistant(StopReason::ToolUse, "", vec![call(&id)]));
        thread.push(results(&id));
    }
    thread.push(assistant(StopReason::ToolUse, "", vec![call("last")]));
    assert_eq!(next(&thread), Step::RunTools(vec![call("last")]));
}

#[test]
fn an_image_message_round_trips_without_integer_array_bytes() {
    let image = crate::Image::new(crate::ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let message = Message::UserImages(crate::UserImages::new("", vec![image]).unwrap());
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("AQID"), "{json}");
    assert!(!json.contains("[1,2,3]"), "{json}");
    assert_eq!(serde_json::from_str::<Message>(&json).unwrap(), message);
    assert_eq!(
        serde_json::from_str::<Message>(r#"{"User":"legacy"}"#).unwrap(),
        Message::User("legacy".to_owned())
    );
    let corrupt = r#"{"UserImages":{"text":"","images":[{"media_type":"image/png","bytes":"","width":0,"height":1}]}}"#;
    assert!(serde_json::from_str::<Message>(corrupt).is_err());
    assert!(serde_json::from_str::<Message>(r#"{"UserImages":{"text":"","images":[]}}"#).is_err());
    assert_eq!(crate::ImageError.code(), "invalid_image");
    let debug = format!("{message:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("1, 2, 3"));
}

#[test]
fn a_thread_cannot_be_restored_empty() {
    assert!(Thread::restore("s", Vec::new()).is_none());
}

#[test]
fn a_restored_thread_reports_the_same_budget_a_live_one_does() {
    let mut live = Thread::new("s", "hello");
    live.push(assistant(StopReason::ToolUse, "", vec![call("c1")]));
    live.push(results("c1"));

    let restored = Thread::restore("s", live.messages().to_vec()).expect("non-empty");
    assert_eq!(restored.steps(), live.steps());
    assert_eq!(next(&restored), next(&live));
}

#[test]
fn a_new_user_prompt_starts_a_new_step_budget() {
    let mut thread = Thread::new("s", "first");
    thread.push(assistant(StopReason::EndTurn, "done", Vec::new()));
    thread.push(Message::User("second".to_owned()));

    assert_eq!(thread.steps(), 0);
    assert_eq!(next(&thread), Step::CallModel);
}

#[test]
fn compaction_does_not_reset_the_active_step_budget() {
    let profile = crate::ExecutionProfile::new(None, None, MAX_STEPS, 0).unwrap();
    let mut thread = Thread::new_with_profile("s", "hello", profile);
    for i in 0..MAX_STEPS - 1 {
        let id = format!("c{i}");
        thread.push(assistant(StopReason::ToolUse, "", vec![call(&id)]));
        thread.push(results(&id));
    }
    thread.compact("summary".to_owned());
    assert_eq!(thread.steps(), MAX_STEPS - 1);

    thread.push(assistant(StopReason::ToolUse, "", vec![call("last")]));
    thread.push(results("last"));
    assert_eq!(thread.steps(), MAX_STEPS);
    assert!(matches!(next(&thread), Step::Done(Outcome::Failed { .. })));
}

#[test]
fn user_turns_after_compaction_reset_live_and_restored_budgets() {
    let image = crate::Image::new(crate::ImageMediaType::Png, vec![1], 1, 1).unwrap();
    for user in [
        Message::User("continue ☕".to_owned()),
        Message::user("", vec![image]),
    ] {
        let mut live = Thread::new("system", "first");
        live.push(assistant(StopReason::ToolUse, "", vec![call("old")]));
        live.push(results("old"));
        live.compact("summary".to_owned());
        assert_eq!(live.steps(), 1);
        live.push(user);
        assert_eq!(live.steps(), 0, "steering starts a fresh budget");
        live.push(assistant(StopReason::ToolUse, "", vec![call("new")]));
        live.push(results("new"));
        let restored = Thread::restore_compacted(
            "system",
            live.messages().to_vec(),
            live.profile().clone(),
            1,
        )
        .unwrap();
        assert_eq!(restored.steps(), 1, "replay ignores pre-steering steps");
        assert_eq!(
            restored.tool_step(),
            2,
            "existing durable tool keys stay stable"
        );
        assert_eq!(restored.steps(), live.steps());
        live.compact("new summary".to_owned());
        assert_eq!(
            live.steps(),
            1,
            "a second compaction preserves this turn only"
        );
    }
}

/// Declining is a structured act, not the absence of output: the model calls
/// [`SILENCE_TOOL`], the run ends with no text, and the call stays in the
/// transcript so the next turn can see that declining is what it did.
#[test]
fn a_silence_call_ends_the_run_with_no_text() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::ToolUse, "", vec![call("c1")]));
    thread.push(silenced("c1", false));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: String::new()
        })
    );
}

/// A `Silence` the registry does not answer to comes back as `unknown_tool`.
/// Ending the run on that would turn a misconfiguration into a dropped reply,
/// so the model sees the error and answers instead.
#[test]
fn a_failed_silence_call_returns_to_the_model() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(StopReason::ToolUse, "", vec![call("c1")]));
    thread.push(silenced("c1", true));
    assert_eq!(next(&thread), Step::CallModel);
}

/// Silence decides the turn even when the model batched other work with it.
/// Every call is still answered first, so the transcript stays valid.
#[test]
fn silence_batched_with_other_work_still_ends_the_run() {
    let mut thread = Thread::new("s", "hello");
    thread.push(assistant(
        StopReason::ToolUse,
        "",
        vec![call("c1"), call("c2")],
    ));
    thread.push(Message::ToolResults(vec![
        result("c1", "search", false),
        result("c2", SILENCE_TOOL, false),
    ]));
    assert_eq!(
        next(&thread),
        Step::Done(Outcome::Completed {
            text: String::new()
        })
    );
}

fn silenced(id: &str, is_error: bool) -> Message {
    Message::ToolResults(vec![result(id, SILENCE_TOOL, is_error)])
}

fn result(id: &str, name: &str, is_error: bool) -> ToolResult {
    ToolResult {
        id: id.to_owned(),
        name: name.to_owned(),
        content: "ok".to_owned(),
        is_error,
        images: Vec::new(),
    }
}
