#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Model selection behavior through the public Admission seam. Each selector
//! is a `/bin/sh -c` script standing in for the deployment's executable.

use std::sync::{Arc, Mutex};

use agent::{Effort, ExecutionProfile};
use brainstone_host::{Admission, ModelSelection, ModelSelector};
use runtime::{Address, MemoryStore, OperationId, Prompt, Store};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Prompt>>>);

impl Admission for Recorder {
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, brainstone_host::Error> {
        self.0.lock().expect("recorder").push(prompt.clone());
        Ok(OperationId::new(1).expect("operation id"))
    }
}

impl Recorder {
    fn profiles(&self) -> Vec<ExecutionProfile> {
        let accepted = self.0.lock().expect("recorder");
        accepted
            .iter()
            .map(|prompt| prompt.profile().clone())
            .collect()
    }
}

fn prompt() -> Prompt {
    let address = Address::new("test", "C1", "thread-1").expect("address");
    let profile = ExecutionProfile::new(None, None, 25, 0).expect("profile");
    Prompt::new(address, "event-1", "be useful", "pourquoi ça échoue ☕")
        .expect("prompt")
        .with_actor("U1")
        .with_profile(profile)
}

fn selector(script: &str) -> ModelSelector {
    ModelSelector::new(
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), script.to_owned()],
    )
}

fn answer(line: &str) -> String {
    format!("cat >/dev/null; printf '%s\\n' '{line}'")
}

#[tokio::test]
async fn an_unconfigured_selector_leaves_the_profile_untouched() {
    let recorder = Recorder::default();
    let selection = ModelSelection::new(recorder.clone(), MemoryStore::default(), None);

    selection.accept(&prompt()).await.expect("admitted");

    assert_eq!(recorder.profiles(), vec![prompt().profile().clone()]);
}

#[tokio::test]
async fn the_selected_model_and_effort_replace_only_those_fields() {
    let recorder = Recorder::default();
    let script = answer(r#"{"model":"claude-haiku-5-5","effort":"xhigh"}"#);
    let selection = ModelSelection::new(
        recorder.clone(),
        MemoryStore::default(),
        Some(selector(&script)),
    );

    selection.accept(&prompt()).await.expect("admitted");

    let expected = ExecutionProfile::new(Some("claude-haiku-5-5"), Some(Effort::XHigh), 25, 0)
        .expect("profile");
    assert_eq!(recorder.profiles(), vec![expected]);
}

#[tokio::test]
async fn the_selector_reads_the_prompt_facts_it_is_promised() {
    // The script answers a model only when every promised field arrived intact.
    let recorder = Recorder::default();
    let script = r#"input=$(cat)
        for field in '"surface":"test"' '"conversation":"C1"' '"lane":"thread-1"' \
            '"actor":"U1"' '"text":"pourquoi ça échoue ☕"'; do
            case "$input" in *"$field"*) ;; *) exit 1;; esac
        done
        printf '%s\n' '{"model":"m","effort":"low"}'"#;
    let selection = ModelSelection::new(
        recorder.clone(),
        MemoryStore::default(),
        Some(selector(script)),
    );

    selection.accept(&prompt()).await.expect("admitted");

    assert_eq!(recorder.profiles()[0].model(), Some("m"));
}

#[tokio::test]
async fn every_unusable_answer_rejects_the_prompt_with_a_stable_code() {
    let cases = [
        answer(r#"{"model":"","effort":"low"}"#),
        answer(r#"{"model":"m","effort":"extreme"}"#),
        answer(r#"{"model":"m"}"#),
        answer(r#"{"effort":"low"}"#),
        answer(r#"{"model":"m","effort":"low","reason":"cheap"}"#),
        answer(r#"["m","low"]"#),
        "cat >/dev/null; printf '%s' '{\"model\":\"m\",\"effort\":\"low\"}'".to_owned(),
        "exit 3".to_owned(),
        "head -c 20000 /dev/zero".to_owned(),
    ];
    for script in cases {
        let recorder = Recorder::default();
        let selection = ModelSelection::new(
            recorder.clone(),
            MemoryStore::default(),
            Some(selector(&script)),
        );

        let error = selection.accept(&prompt()).await.unwrap_err();

        assert_eq!(error.code(), "model_selection_failed", "{script}");
        assert!(recorder.profiles().is_empty(), "{script}");
    }
}

#[tokio::test]
async fn a_redelivered_request_skips_the_selector() {
    // The original admission already stored its profile; a broken selector
    // must not turn a duplicate delivery into a rejection.
    let store = MemoryStore::default();
    store.accept(&prompt()).await.expect("first delivery");
    let recorder = Recorder::default();
    let selection = ModelSelection::new(recorder.clone(), store, Some(selector("exit 1")));

    selection
        .accept(&prompt())
        .await
        .expect("duplicate admitted");

    assert_eq!(recorder.profiles(), vec![prompt().profile().clone()]);
}

fn contracted_profile(legacy: bool) -> ExecutionProfile {
    let schema = agent::OutputSchema::new(
        "answer",
        serde_json::json!({
            "type":"object", "properties":{"value":{"type":"string"}},
            "required":["value"], "additionalProperties":false
        }),
    )
    .unwrap();
    let profile = ExecutionProfile::new(None, None, 7, 2)
        .unwrap()
        .with_allowed_agents(Some(vec!["reviewer".to_owned()]))
        .with_output_schema(schema);
    let mut value = serde_json::to_value(profile).unwrap();
    if legacy {
        value.as_object_mut().unwrap().remove("tool_key_version");
    }
    serde_json::from_value(value).unwrap()
}

async fn select_profile(profile: ExecutionProfile) -> ExecutionProfile {
    let recorder = Recorder::default();
    let script = answer(r#"{"model":"selected","effort":"high"}"#);
    let selection = ModelSelection::new(
        recorder.clone(),
        MemoryStore::default(),
        Some(selector(&script)),
    );
    selection
        .accept(&prompt().with_profile(profile))
        .await
        .unwrap();
    recorder.profiles().pop().unwrap()
}

#[tokio::test]
async fn selection_preserves_schema_retry_version_and_all_other_profile_fields() {
    for legacy in [false, true] {
        let profile = contracted_profile(legacy);
        let mut expected = serde_json::to_value(&profile).unwrap();
        expected["model"] = serde_json::json!("selected");
        expected["effort"] = serde_json::json!("high");
        let selected = select_profile(profile).await;
        assert_eq!(serde_json::to_value(selected).unwrap(), expected);
    }
}
