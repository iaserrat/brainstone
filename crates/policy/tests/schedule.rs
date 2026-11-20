#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Durable scheduling through the public tool and runtime seams.
#![allow(clippy::too_many_lines)]

use std::sync::Mutex;

use agent::{Model, ModelResponse, Registry, StopReason, Thread, ToolCall, ToolContext};
use runtime::{Address, MemoryStore, Prompt, Runtime};
use serde_json::json;

struct Capture(Mutex<Vec<String>>);

const FLOOR: u32 = brainstone_policy::DEFAULT_MIN_INTERVAL_MINUTES;

/// A surface that lets its requester post in conversation `C9` and nowhere else.
struct Eng;

impl surface::Destinations for Eng {
    async fn resolve(
        &self,
        request: surface::PostRequest<'_>,
    ) -> Result<runtime::Destination, agent::ToolError> {
        if request.conversation != "C9" {
            return Err(agent::ToolError::new(
                "post_conversation_unavailable",
                "not here",
            ));
        }
        let identity = agent::ExecutionIdentity {
            conversation: "C9".into(),
            private: false,
            ..request.identity.clone()
        };
        Ok(runtime::Destination {
            surface: "test".into(),
            conversation: identity.conversation_key(),
            target: runtime::DeliveryTarget::new("test", json!({"post": "C9"})).unwrap(),
            identity,
        })
    }
}

#[test]
fn schedule_definition_avoids_provider_rejected_schema_combinators() {
    let tools =
        brainstone_policy::registry(MemoryStore::default(), &Registry::default(), FLOOR, Eng);
    let schedule = tools
        .definitions()
        .into_iter()
        .find(|definition| definition["name"] == "Schedule")
        .expect("schedule definition");

    assert!(schedule["input_schema"].get("oneOf").is_none());
    assert!(schedule["input_schema"].get("allOf").is_none());
    assert!(schedule["input_schema"].get("anyOf").is_none());
}

#[tokio::test]
async fn schedule_still_requires_exactly_one_timing_mode() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let origin = runtime.accept(&prompt()).await.unwrap();
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let context = ToolContext {
        identity: None,
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };
    let result = execute(
        &tools,
        "Schedule",
        json!({"prompt": "later", "after_seconds": 1, "repeat_time": "08:00"}),
        Some("invalid-schedule"),
        context,
    )
    .await;

    assert!(result.is_error);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.content).unwrap()["error"],
        "invalid_schedule_arguments"
    );
}

#[tokio::test]
async fn a_standing_schedule_needs_a_zone_and_reports_its_recurrence() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let origin = runtime.accept(&prompt()).await.unwrap();
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let context = ToolContext {
        identity: None,
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };
    let missing_zone = execute(
        &tools,
        "Schedule",
        json!({"prompt": "brief me", "repeat_time": "08:00"}),
        Some("k1"),
        context.clone(),
    )
    .await;
    assert!(missing_zone.is_error);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&missing_zone.content).unwrap()["error"],
        "invalid_schedule_arguments"
    );
    let bad_zone = execute(
        &tools,
        "Schedule",
        json!({"prompt": "brief me", "repeat_time": "08:00", "repeat_time_zone": "Mars/Olympus"}),
        Some("k2"),
        context.clone(),
    )
    .await;
    assert!(bad_zone.is_error);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&bad_zone.content).unwrap()["error"],
        "invalid_input"
    );
    let standing = execute(
        &tools,
        "Schedule",
        json!({
            "prompt": "brief me",
            "repeat_time": "08:00",
            "repeat_time_zone": "America/Toronto",
            "repeat_days": [1, 2, 3, 4, 5]
        }),
        Some("k3"),
        context.clone(),
    )
    .await;
    assert!(!standing.is_error, "{}", standing.content);
    let standing: serde_json::Value = serde_json::from_str(&standing.content).unwrap();
    assert_eq!(standing["repeat"]["time"], "08:00:00");
    assert_eq!(standing["repeat"]["time_zone"], "America/Toronto");
    assert_eq!(standing["repeat"]["days"], json!([1, 2, 3, 4, 5]));
    assert_eq!(standing["state"], "pending");
    let listed = execute(&tools, "ListSchedules", json!({}), None, context).await;
    let listed: serde_json::Value = serde_json::from_str(&listed.content).unwrap();
    assert_eq!(listed[0]["repeat"]["time_zone"], "America/Toronto");
}

#[test]
fn schedule_advertises_the_operator_floor_and_no_self_rescheduling_rule() {
    let tools = brainstone_policy::registry(MemoryStore::default(), &Registry::default(), 30, Eng);
    let schedule = tools
        .definitions()
        .into_iter()
        .find(|definition| definition["name"] == "Schedule")
        .expect("schedule definition");
    assert_eq!(
        schedule["input_schema"]["properties"]["repeat_every_minutes"]["minimum"],
        30
    );
    let description = schedule["description"].as_str().unwrap();
    assert!(!description.contains("reschedule"), "{description}");
}

async fn standing_context(store: &MemoryStore) -> ToolContext {
    let runtime = Runtime::new(store.clone());
    let origin = runtime.accept(&prompt()).await.unwrap();
    ToolContext {
        identity: None,
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    }
}

fn error_code(result: &agent::ToolResult) -> serde_json::Value {
    assert!(result.is_error, "{}", result.content);
    serde_json::from_str::<serde_json::Value>(&result.content).unwrap()["error"].clone()
}

#[tokio::test]
async fn an_interval_below_the_operator_floor_is_refused() {
    let store = MemoryStore::default();
    let context = standing_context(&store).await;
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let every =
        |minutes: u32| json!({"prompt": "check the queue", "repeat_every_minutes": minutes});
    let short = execute(
        &tools,
        "Schedule",
        every(FLOOR - 1),
        Some("k1"),
        context.clone(),
    )
    .await;
    assert_eq!(error_code(&short), "schedule_interval_too_short");
    let at_floor = execute(
        &tools,
        "Schedule",
        every(FLOOR),
        Some("k2"),
        context.clone(),
    )
    .await;
    assert!(!at_floor.is_error, "{}", at_floor.content);
    let at_floor: serde_json::Value = serde_json::from_str(&at_floor.content).unwrap();
    assert_eq!(at_floor["repeat"], json!({"every_minutes": FLOOR}));
    let mixed = json!({
        "prompt": "check", "repeat_every_minutes": 60, "repeat_time": "09:00",
        "repeat_time_zone": "America/Toronto"
    });
    let mixed = execute(&tools, "Schedule", mixed, Some("k3"), context.clone()).await;
    assert_eq!(error_code(&mixed), "invalid_schedule_arguments");
    let zoned = json!({
        "prompt": "check", "repeat_every_minutes": 60, "repeat_time_zone": "America/Toronto"
    });
    let zoned = execute(&tools, "Schedule", zoned, Some("k4"), context).await;
    assert_eq!(error_code(&zoned), "invalid_input");
}

#[tokio::test]
async fn calendar_rules_are_stored_as_written() {
    let store = MemoryStore::default();
    let context = standing_context(&store).await;
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let first_monday = json!({
        "prompt": "post the plan", "repeat_time": "09:00", "repeat_time_zone": "America/Toronto",
        "repeat_days": [1], "repeat_nth": 1
    });
    let first_monday = execute(
        &tools,
        "Schedule",
        first_monday,
        Some("k1"),
        context.clone(),
    )
    .await;
    assert!(!first_monday.is_error, "{}", first_monday.content);
    let first_monday: serde_json::Value = serde_json::from_str(&first_monday.content).unwrap();
    assert_eq!(
        first_monday["repeat"],
        json!({"time": "09:00:00", "time_zone": "America/Toronto", "days": [1], "nth": 1})
    );
    let fortnightly = json!({
        "prompt": "retro", "repeat_time": "16:30", "repeat_time_zone": "Europe/Lisbon",
        "repeat_days": [5], "repeat_interval": 2
    });
    let fortnightly = execute(&tools, "Schedule", fortnightly, Some("k2"), context.clone()).await;
    let fortnightly: serde_json::Value = serde_json::from_str(&fortnightly.content).unwrap();
    assert_eq!(fortnightly["repeat"]["interval"], 2);
    let fifth = json!({
        "prompt": "x", "repeat_time": "09:00", "repeat_time_zone": "UTC",
        "repeat_days": [1], "repeat_nth": 5
    });
    let fifth = execute(&tools, "Schedule", fifth, Some("k3"), context).await;
    assert_eq!(error_code(&fifth), "invalid_input");
}

#[tokio::test]
async fn a_destination_posts_where_the_surface_allows_and_only_for_a_known_requester() {
    let store = MemoryStore::default();
    let owner = agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: "U1".into(),
        conversation: "D1".into(),
        private: true,
    };
    let address = Address::new("test", &owner.conversation_key(), "d1").unwrap();
    let origin = Prompt::new(
        address,
        "origin",
        "be useful",
        "post the plan in the eng room",
    )
    .unwrap()
    .with_identity(Some(owner.clone()))
    .with_delivery(runtime::DeliveryTarget::new("test", json!({"post": "d1"})).unwrap());
    let origin = Runtime::new(store.clone()).accept(&origin).await.unwrap();
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let context = ToolContext {
        identity: Some(owner),
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };
    let post = |conversation: &str| json!({"prompt": "post the plan", "after_seconds": 0, "conversation": conversation});
    let posted = execute(&tools, "Schedule", post("C9"), Some("k1"), context.clone()).await;
    assert!(!posted.is_error, "{}", posted.content);
    let posted: serde_json::Value = serde_json::from_str(&posted.content).unwrap();
    assert_eq!(posted["destination"], "C9");
    let refused = execute(&tools, "Schedule", post("C1"), Some("k2"), context.clone()).await;
    assert_eq!(
        error_code(&refused),
        "post_conversation_unavailable",
        "the surface decides"
    );
    let anonymous = ToolContext {
        identity: None,
        ..context
    };
    let anonymous = execute(&tools, "Schedule", post("C9"), Some("k3"), anonymous).await;
    assert_eq!(error_code(&anonymous), "schedule_destination_forbidden");
}

impl Model for Capture {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        self.0.lock().unwrap().push(match thread.last() {
            agent::Message::User(text) => text.clone(),
            other => panic!("expected user prompt, got {other:?}"),
        });
        response("done")
    }
}

#[tokio::test]
async fn a_due_schedule_survives_restart_and_is_released_once() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let origin = runtime.accept(&prompt()).await.unwrap();
    runtime
        .run_next(&Capture(Mutex::new(Vec::new())), &Registry::default())
        .await
        .unwrap();
    let tools = brainstone_policy::registry(store.clone(), &Registry::default(), FLOOR, Eng);
    let call = ToolCall {
        id: "call-1".to_owned(),
        name: "Schedule".to_owned(),
        args: json!({"prompt": "check the deployment", "after_seconds": 0}),
    };
    let context = ToolContext {
        identity: None,
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };
    let first = agent::execute_tool_with_context(
        &call,
        &tools,
        Some("stable-schedule-key".to_owned()),
        Some(context.clone()),
    )
    .await;
    let retry = agent::execute_tool_with_context(
        &call,
        &tools,
        Some("stable-schedule-key".to_owned()),
        Some(context),
    )
    .await;
    assert!(!first.is_error);
    assert_eq!(first.content, retry.content);
    drop(runtime);

    let capture = Capture(Mutex::new(Vec::new()));
    let reopened = Runtime::new(store);
    reopened
        .run_next(&capture, &tools)
        .await
        .unwrap()
        .expect("scheduled operation");
    assert_eq!(*capture.0.lock().unwrap(), vec!["check the deployment"]);
    assert!(reopened.run_next(&capture, &tools).await.unwrap().is_none());
}

#[tokio::test]
async fn scheduled_work_can_be_listed_and_cancelled_idempotently() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let origin = runtime.accept(&prompt()).await.unwrap();
    runtime
        .run_next(&Capture(Mutex::new(Vec::new())), &Registry::default())
        .await
        .unwrap();
    let tools = brainstone_policy::registry(store, &Registry::default(), FLOOR, Eng);
    let context = ToolContext {
        identity: None,
        operation_id: u64::try_from(origin.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };
    let scheduled = execute(
        &tools,
        "Schedule",
        json!({"prompt": "later", "after_seconds": 0}),
        Some("schedule-key"),
        context.clone(),
    )
    .await;
    let id = serde_json::from_str::<serde_json::Value>(&scheduled.content).unwrap()["id"]
        .as_i64()
        .unwrap();

    let listed = execute(&tools, "ListSchedules", json!({}), None, context.clone()).await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&listed.content).unwrap()[0]["prompt"],
        "later"
    );
    let first = execute(
        &tools,
        "CancelSchedule",
        json!({"id": id}),
        None,
        context.clone(),
    )
    .await;
    let retry = execute(
        &tools,
        "CancelSchedule",
        json!({"id": id}),
        None,
        context.clone(),
    )
    .await;
    assert_eq!(first.content, retry.content);
    let listed = execute(&tools, "ListSchedules", json!({}), None, context).await;
    assert_eq!(listed.content, "[]");
    assert!(
        runtime
            .run_next(&Capture(Mutex::new(Vec::new())), &tools)
            .await
            .unwrap()
            .is_none()
    );
}

async fn execute(
    tools: &Registry,
    name: &str,
    args: serde_json::Value,
    key: Option<&str>,
    context: ToolContext,
) -> agent::ToolResult {
    let call = ToolCall {
        id: format!("{name}-call"),
        name: name.to_owned(),
        args,
    };
    agent::execute_tool_with_context(&call, tools, key.map(str::to_owned), Some(context)).await
}

fn prompt() -> Prompt {
    let address = Address::new("test", "C123", "thread-42").unwrap();
    Prompt::new(address, "origin", "be useful", "start").unwrap()
}

fn response(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    }
}
