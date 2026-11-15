use brainstone::{ConfigError, CoreSettings};
use std::{collections::HashMap, path::Path};

#[test]
fn local_models_use_neutral_names_and_need_no_legacy_provider_credentials() {
    let settings = CoreSettings::from_map(&local(), &root()).unwrap();
    assert_eq!(settings.anthropic_model(), "local-model");
}

/// Summaries are a Responses request field, so other protocols and unattached
/// settings are rejected instead of being silently ignored.
#[test]
fn reasoning_summaries_are_opt_in_for_responses_targets_only() {
    let mut responses = local();
    responses.insert("MODEL_PROTOCOL".to_owned(), "responses".to_owned());
    responses.insert("MODEL_REASONING_SUMMARY".to_owned(), "true".to_owned());
    assert!(CoreSettings::from_map(&responses, &root()).is_ok());

    let invalid = ConfigError::Invalid {
        name: "MODEL_REASONING_SUMMARY",
    };
    let mut chat = local();
    chat.insert("MODEL_REASONING_SUMMARY".to_owned(), "true".to_owned());
    assert_eq!(CoreSettings::from_map(&chat, &root()).unwrap_err(), invalid);
    responses.insert("MODEL_REASONING_SUMMARY".to_owned(), "yes".to_owned());
    assert_eq!(
        CoreSettings::from_map(&responses, &root()).unwrap_err(),
        invalid
    );

    let mut orphaned = local();
    orphaned.remove("MODEL_ENDPOINT");
    orphaned.insert("MODEL_REASONING_SUMMARY".to_owned(), "true".to_owned());
    assert_eq!(
        CoreSettings::from_map(&orphaned, &root()).unwrap_err(),
        ConfigError::Missing {
            name: "MODEL_ENDPOINT"
        }
    );
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/minimal-agent")
}

fn local() -> HashMap<String, String> {
    HashMap::from([
        ("DATABASE_URL".to_owned(), "sqlite://agent.db".to_owned()),
        ("WORKER_ID".to_owned(), "local".to_owned()),
        ("MODEL_PROVIDER".to_owned(), "openai".to_owned()),
        ("MODEL_NAME".to_owned(), "local-model".to_owned()),
        (
            "MODEL_ENDPOINT".to_owned(),
            "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
        ),
        ("MODEL_PROTOCOL".to_owned(), "chat-completions".to_owned()),
        ("MODEL_TARGET".to_owned(), "local".to_owned()),
        ("MODEL_AUTH".to_owned(), "none".to_owned()),
    ])
}
