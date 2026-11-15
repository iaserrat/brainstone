//! Table-driven deployment configuration contract.
#![allow(clippy::too_many_lines)]

use std::collections::HashMap;
use std::path::Path;

use anthropic::Effort;
use brainstone::{BedrockApi, ConfigError, CoreSettings, ModelProvider, Settings};

#[test]
fn deployment_settings_are_validated_and_secrets_are_redacted() {
    let mut values = HashMap::from([
        (
            "DATABASE_URL".to_owned(),
            "postgres://db/brainstone".to_owned(),
        ),
        (
            "ANTHROPIC_API_KEY".to_owned(),
            "anthropic-secret".to_owned(),
        ),
        ("ANTHROPIC_MODEL".to_owned(), "claude-test".to_owned()),
        ("ANTHROPIC_MAX_TOKENS".to_owned(), "8192".to_owned()),
        ("MODEL_EFFORT".to_owned(), "xhigh".to_owned()),
        ("WORKER_ID".to_owned(), "replica-1".to_owned()),
    ]);
    values.insert(
        "AGENT_SYSTEM_PROMPT".to_owned(),
        "this obsolete value must be ignored".to_owned(),
    );

    let settings = Settings::from_map(&values, workspace_root()).unwrap();

    assert_eq!(
        settings.model_provider(),
        &ModelProvider::Anthropic {
            api_key: "anthropic-secret".to_owned()
        }
    );
    assert_eq!(settings.anthropic_model(), "claude-test");
    assert_eq!(settings.anthropic_max_tokens(), 8192);
    assert_eq!(settings.model_effort(), Effort::XHigh);
    assert_eq!(settings.auto_compact_token_limit(), 180_000);
    assert_eq!(
        settings.system_prompt(),
        "You are a useful assistant.\n\nFollow the user's request.\n"
    );
    let debug = format!("{settings:?}");
    assert!(!debug.contains("anthropic-secret"));

    let mut missing = values;
    missing.remove("DATABASE_URL");
    let error = Settings::from_map(&missing, workspace_root()).unwrap_err();
    assert_eq!(
        error,
        ConfigError::Missing {
            name: "DATABASE_URL"
        }
    );
    assert_eq!(error.code(), "invalid_configuration");
}

#[test]
fn model_turn_limit_defaults_to_unlimited() {
    let settings = Settings::from_map(&valid_values(), workspace_root()).unwrap();

    assert_eq!(settings.max_turns(), None);
}

#[test]
fn model_turn_limit_accepts_explicit_unlimited() {
    let mut values = valid_values();
    values.insert("MAX_TURNS".to_owned(), "unlimited".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(settings.max_turns(), None);
}

#[test]
fn model_turn_limit_accepts_a_positive_override() {
    let mut values = valid_values();
    values.insert("MAX_TURNS".to_owned(), "250".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();

    assert_eq!(settings.max_turns(), Some(250));
}

#[test]
fn model_turn_limit_rejects_non_positive_or_malformed_values() {
    for value in ["0", "many", "-1", "", "1.5", "18446744073709551616"] {
        let mut values = valid_values();
        values.insert("MAX_TURNS".to_owned(), value.to_owned());

        assert_eq!(
            Settings::from_map(&values, workspace_root()).unwrap_err(),
            ConfigError::Invalid { name: "MAX_TURNS" }
        );
    }
}

#[test]
fn schedule_interval_floor_defaults_and_accepts_only_positive_minutes() {
    let settings = Settings::from_map(&valid_values(), workspace_root()).unwrap();
    assert_eq!(
        settings.core().schedule_min_interval_minutes(),
        policy::DEFAULT_MIN_INTERVAL_MINUTES
    );
    let mut values = valid_values();
    values.insert("SCHEDULE_MIN_INTERVAL_MINUTES".to_owned(), "60".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(settings.core().schedule_min_interval_minutes(), 60);
    for value in ["0", "-5", "soon", "4294967296"] {
        values.insert("SCHEDULE_MIN_INTERVAL_MINUTES".to_owned(), value.to_owned());
        assert_eq!(
            Settings::from_map(&values, workspace_root()).unwrap_err(),
            ConfigError::Invalid {
                name: "SCHEDULE_MIN_INTERVAL_MINUTES"
            },
            "{value}"
        );
    }
}

#[test]
fn compaction_limit_defaults_to_ninety_percent_and_clamps_overrides() {
    let mut values = valid_values();
    values.insert("MODEL_CONTEXT_WINDOW".to_owned(), "1000".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root())
            .unwrap()
            .auto_compact_token_limit(),
        900
    );

    values.insert("AUTO_COMPACT_TOKEN_LIMIT".to_owned(), "950".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root())
            .unwrap()
            .auto_compact_token_limit(),
        900
    );
    values.insert("AUTO_COMPACT_TOKEN_LIMIT".to_owned(), "0".to_owned());
    assert!(matches!(
        Settings::from_map(&values, workspace_root()),
        Err(ConfigError::Invalid {
            name: "AUTO_COMPACT_TOKEN_LIMIT"
        })
    ));
}

fn workspace_root() -> &'static Path {
    Box::leak(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/minimal-agent")
            .into_boxed_path(),
    )
}

#[test]
fn model_effort_defaults_to_high_and_rejects_unknown_values() {
    let mut values = valid_values();
    assert_eq!(
        Settings::from_map(&values, workspace_root())
            .unwrap()
            .model_effort(),
        Effort::High
    );

    values.insert("MODEL_EFFORT".to_owned(), "very-high".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Invalid {
            name: "MODEL_EFFORT"
        }
    );
}

#[test]
fn system_prompt_file_is_required_readable_and_nonblank() {
    let values = valid_values();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    let missing = Settings::from_map(&values, &fixture.join("missing")).unwrap_err();
    assert_eq!(
        missing,
        ConfigError::Missing {
            name: "prompts/system_prompt.md"
        }
    );
    assert_eq!(missing.code(), "invalid_configuration");

    let blank = Settings::from_map(&values, &fixture.join("blank")).unwrap_err();
    assert_eq!(
        blank,
        ConfigError::Invalid {
            name: "prompts/system_prompt.md"
        }
    );

    let unreadable = Settings::from_map(&values, &fixture.join("unreadable")).unwrap_err();
    assert_eq!(
        unreadable,
        ConfigError::Unreadable {
            name: "prompts/system_prompt.md"
        }
    );
    assert_eq!(unreadable.code(), "invalid_configuration");
}

#[test]
fn mcp_configuration_reports_the_actual_cause_without_secrets() {
    let values = valid_values();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid_mcp");
    let error = Settings::from_map(&values, &fixture).unwrap_err();

    assert_eq!(error.code(), "invalid_configuration");
    assert!(error.to_string().contains("MCP_MISSING_TOKEN"));
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn subagents_are_loaded_from_the_workspace_agents_directory() {
    let values = valid_values();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/agent_root");

    let error = Settings::from_map(&values, &fixture).unwrap_err();

    assert_eq!(error.code(), "invalid_configuration");
    assert!(error.to_string().contains("agents"));
}

fn valid_values() -> HashMap<String, String> {
    HashMap::from([
        (
            "DATABASE_URL".to_owned(),
            "postgres://db/brainstone".to_owned(),
        ),
        ("ANTHROPIC_API_KEY".to_owned(), "secret".to_owned()),
        ("ANTHROPIC_MODEL".to_owned(), "claude-test".to_owned()),
        ("ANTHROPIC_MAX_TOKENS".to_owned(), "8192".to_owned()),
        ("WORKER_ID".to_owned(), "replica-1".to_owned()),
    ])
}

#[test]
fn admission_defaults_to_open_and_parses_explicit_limits() {
    let settings = Settings::from_map(&valid_values(), workspace_root()).unwrap();
    assert_eq!(settings.allowed_actors(), None);
    assert_eq!(settings.actor_daily_limit(), None);

    let mut values = valid_values();
    values.insert(
        "ADMISSION_ALLOWED_ACTORS".to_owned(),
        "U1, U2 ,U3".to_owned(),
    );
    values.insert("ADMISSION_ACTOR_DAILY_LIMIT".to_owned(), "20".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    let actors: Vec<&str> = settings
        .allowed_actors()
        .unwrap()
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(actors, vec!["U1", "U2", "U3"]);
    assert_eq!(settings.actor_daily_limit(), Some(20));

    for (name, value) in [
        ("ADMISSION_ALLOWED_ACTORS", " , "),
        ("ADMISSION_ACTOR_DAILY_LIMIT", "0"),
        ("ADMISSION_ACTOR_DAILY_LIMIT", "many"),
    ] {
        let mut invalid = valid_values();
        invalid.insert(name.to_owned(), value.to_owned());
        let error = Settings::from_map(&invalid, workspace_root()).unwrap_err();
        assert_eq!(error, ConfigError::Invalid { name });
    }
}

#[test]
fn memory_defaults_to_off_and_parses_explicit_triggers() {
    let settings = Settings::from_map(&valid_values(), workspace_root()).unwrap();
    assert_eq!(settings.memory(), &memory::Config::default());

    let mut values = valid_values();
    values.insert("MEMORY_SAVE".to_owned(), "true".to_owned());
    values.insert("MEMORY_RECALL_TOOL".to_owned(), "true".to_owned());
    values.insert("MEMORY_INJECT".to_owned(), "5".to_owned());
    values.insert("MEMORY_RECALL_LIMIT".to_owned(), "3".to_owned());
    values.insert("MEMORY_SCOPE".to_owned(), "surface".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    let config = settings.memory();
    assert!(config.save);
    assert!(config.recall_tool);
    assert_eq!(config.inject, Some(5));
    assert_eq!(config.recall_limit, 3);
    assert_eq!(config.scope, memory::Scope::Surface);

    values.insert("MEMORY_SCOPE".to_owned(), "user".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(settings.memory().scope, memory::Scope::User);

    for (name, value) in [
        ("MEMORY_SAVE", "yes"),
        ("MEMORY_RECALL_TOOL", "1"),
        ("MEMORY_SCOPE", "global"),
        ("MEMORY_INJECT", "0"),
        ("MEMORY_RECALL_LIMIT", "none"),
    ] {
        let mut invalid = valid_values();
        invalid.insert(name.to_owned(), value.to_owned());
        let error = Settings::from_map(&invalid, workspace_root()).unwrap_err();
        assert_eq!(error, ConfigError::Invalid { name });
    }
}

#[test]
fn model_provider_selects_anthropic_or_bedrock_with_bearer_or_iam_auth() {
    let mut values = valid_values();
    values.insert("MODEL_PROVIDER".to_owned(), "bedrock".to_owned());
    values.insert("AWS_REGION".to_owned(), "eu-west-1".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(
        settings.model_provider(),
        &ModelProvider::Bedrock {
            api: BedrockApi::Runtime,
            region: "eu-west-1".to_owned(),
            api_key: Some("secret".to_owned()),
            endpoint_url: None,
        }
    );
    assert!(!format!("{settings:?}").contains("secret"));

    values.remove("ANTHROPIC_API_KEY");
    values.insert("MODEL_PROVIDER".to_owned(), "bedrock-mantle".to_owned());
    values.insert(
        "AWS_ENDPOINT_URL_BEDROCK_RUNTIME".to_owned(),
        "https://vpce.example/".to_owned(),
    );
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(
        settings.model_provider(),
        &ModelProvider::Bedrock {
            api: BedrockApi::Mantle,
            region: "eu-west-1".to_owned(),
            api_key: None,
            endpoint_url: Some("https://vpce.example/".to_owned()),
        }
    );

    values.insert("ANTHROPIC_API_KEY".to_owned(), " ".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Invalid {
            name: "ANTHROPIC_API_KEY"
        }
    );

    values.remove("AWS_REGION");
    values.remove("ANTHROPIC_API_KEY");
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Missing { name: "AWS_REGION" }
    );

    let mut values = valid_values();
    values.remove("ANTHROPIC_API_KEY");
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Missing {
            name: "ANTHROPIC_API_KEY"
        }
    );

    let mut values = valid_values();
    values.insert("MODEL_PROVIDER".to_owned(), "vertex".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Invalid {
            name: "MODEL_PROVIDER"
        }
    );
}

#[test]
fn fallback_model_is_optional_validated_and_redacted() {
    let settings = Settings::from_map(&valid_values(), workspace_root()).unwrap();
    assert!(settings.core().fallback().is_none());

    let mut values = valid_values();
    values.insert("FALLBACK_MODEL".to_owned(), "claude-test".to_owned());
    values.insert("FALLBACK_MODEL_PROVIDER".to_owned(), "bedrock".to_owned());
    values.insert("FALLBACK_AWS_REGION".to_owned(), "us-east-1".to_owned());
    values.insert(
        "FALLBACK_ANTHROPIC_API_KEY".to_owned(),
        "fallback-secret".to_owned(),
    );
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    let fallback = settings.core().fallback().expect("configured");
    assert_eq!(fallback.model(), "claude-test");
    assert_eq!(
        fallback.provider(),
        &ModelProvider::Bedrock {
            api: BedrockApi::Runtime,
            region: "us-east-1".to_owned(),
            api_key: Some("fallback-secret".to_owned()),
            endpoint_url: None,
        }
    );
    assert!(!format!("{settings:?}").contains("fallback-secret"));

    let mut orphaned = values.clone();
    orphaned.remove("FALLBACK_MODEL");
    assert_eq!(
        Settings::from_map(&orphaned, workspace_root()).unwrap_err(),
        ConfigError::Missing {
            name: "FALLBACK_MODEL"
        }
    );

    let mut regionless = values.clone();
    regionless.remove("FALLBACK_AWS_REGION");
    assert_eq!(
        Settings::from_map(&regionless, workspace_root()).unwrap_err(),
        ConfigError::Missing {
            name: "FALLBACK_AWS_REGION"
        }
    );

    let mut unknown = values.clone();
    unknown.insert("FALLBACK_MODEL_PROVIDER".to_owned(), "vertex".to_owned());
    assert_eq!(
        Settings::from_map(&unknown, workspace_root()).unwrap_err(),
        ConfigError::Invalid {
            name: "FALLBACK_MODEL_PROVIDER"
        }
    );

    let mut identical = valid_values();
    identical.insert("FALLBACK_MODEL".to_owned(), "claude-test".to_owned());
    identical.insert(
        "FALLBACK_ANTHROPIC_API_KEY".to_owned(),
        "other-key".to_owned(),
    );
    let error = Settings::from_map(&identical, workspace_root()).unwrap_err();
    assert!(matches!(
        error,
        ConfigError::Detailed {
            name: "FALLBACK_MODEL",
            ..
        }
    ));
}

#[test]
fn openai_providers_parse_for_the_primary_and_the_fallback() {
    let mut values = valid_values();
    values.insert("MODEL_PROVIDER".to_owned(), "openai".to_owned());
    assert_eq!(
        Settings::from_map(&values, workspace_root()).unwrap_err(),
        ConfigError::Missing {
            name: "OPENAI_API_KEY"
        }
    );
    values.insert("OPENAI_API_KEY".to_owned(), "openai-secret".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    assert_eq!(
        settings.model_provider(),
        &ModelProvider::OpenAi {
            api_key: "openai-secret".to_owned()
        }
    );
    assert!(!format!("{settings:?}").contains("openai-secret"));

    let mut values = valid_values();
    values.insert("FALLBACK_MODEL".to_owned(), "us.openai.gpt-test".to_owned());
    values.insert(
        "FALLBACK_MODEL_PROVIDER".to_owned(),
        "bedrock-openai".to_owned(),
    );
    values.insert("FALLBACK_AWS_REGION".to_owned(), "us-east-1".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    let fallback = settings.core().fallback().expect("configured");
    assert_eq!(
        fallback.provider(),
        &ModelProvider::Bedrock {
            api: BedrockApi::OpenAiRuntime,
            region: "us-east-1".to_owned(),
            api_key: None,
            endpoint_url: None,
        }
    );
    assert_eq!(fallback.max_tokens(), 8192, "inherits ANTHROPIC_MAX_TOKENS");
    assert_eq!(fallback.effort(), Effort::High, "inherits MODEL_EFFORT");
}

#[test]
fn fallback_effort_and_output_cap_override_the_primary() {
    let mut values = valid_values();
    values.insert("MODEL_EFFORT".to_owned(), "max".to_owned());
    values.insert("FALLBACK_MODEL".to_owned(), "gpt-test".to_owned());
    values.insert("FALLBACK_MODEL_PROVIDER".to_owned(), "openai".to_owned());
    values.insert("FALLBACK_OPENAI_API_KEY".to_owned(), "k".to_owned());
    values.insert("FALLBACK_MODEL_EFFORT".to_owned(), "medium".to_owned());
    values.insert("FALLBACK_MAX_TOKENS".to_owned(), "32000".to_owned());
    let settings = Settings::from_map(&values, workspace_root()).unwrap();
    let fallback = settings.core().fallback().expect("configured");
    assert_eq!(fallback.effort(), Effort::Medium);
    assert_eq!(fallback.max_tokens(), 32_000);
    assert_eq!(settings.model_effort(), Effort::Max);

    for (name, value) in [
        ("FALLBACK_MODEL_EFFORT", "minimal"),
        ("FALLBACK_MAX_TOKENS", "0"),
    ] {
        let mut invalid = values.clone();
        invalid.insert(name.to_owned(), value.to_owned());
        assert_eq!(
            Settings::from_map(&invalid, workspace_root()).unwrap_err(),
            ConfigError::Invalid { name }
        );
    }
    let mut orphaned = valid_values();
    orphaned.insert("FALLBACK_MODEL_EFFORT".to_owned(), "low".to_owned());
    assert_eq!(
        Settings::from_map(&orphaned, workspace_root()).unwrap_err(),
        ConfigError::Missing {
            name: "FALLBACK_MODEL"
        }
    );
}

/// Cache writes cost more than plain input, so an upgrade never turns them on.
#[test]
fn prompt_cache_defaults_off_and_turns_on_with_true() {
    let mut values = valid_values();
    let settings = |values: &HashMap<String, String>| Settings::from_map(values, workspace_root());
    assert!(!settings(&values).unwrap().prompt_cache());
    values.insert("PROMPT_CACHE".to_owned(), "true".to_owned());
    assert!(settings(&values).unwrap().prompt_cache());
    values.insert("PROMPT_CACHE".to_owned(), "yes".to_owned());
    assert_eq!(
        settings(&values).unwrap_err(),
        ConfigError::Invalid {
            name: "PROMPT_CACHE"
        }
    );
}

/// An upgrade never adds a tool to an operator's catalog unasked.
#[test]
fn the_file_tool_defaults_off_and_turns_on_with_true() {
    let mut values = valid_values();
    let settings = |values: &HashMap<String, String>| Settings::from_map(values, workspace_root());
    assert!(!settings(&values).unwrap().file_tool());
    values.insert("FILE_TOOL".to_owned(), "true".to_owned());
    assert!(settings(&values).unwrap().file_tool());
    values.insert("FILE_TOOL".to_owned(), "1".to_owned());
    assert_eq!(
        settings(&values).unwrap_err(),
        ConfigError::Invalid { name: "FILE_TOOL" }
    );
}

#[test]
fn dynamic_model_selection_is_off_by_default_and_needs_both_variables() {
    const COMMAND: &str = "DYNAMIC_MODEL_AND_EFFORT_COMMAND";
    const ARGS: &str = "DYNAMIC_MODEL_AND_EFFORT_ARGS";
    let off = CoreSettings::from_map(&valid_values(), workspace_root()).unwrap();
    assert_eq!(off.model_selector(), None);

    let mut on = valid_values();
    on.insert(COMMAND.to_owned(), "/usr/bin/env".to_owned());
    on.insert(ARGS.to_owned(), r#"["node","selector.mjs"]"#.to_owned());
    let expected = host::ModelSelector::new(
        "/usr/bin/env".to_owned(),
        vec!["node".to_owned(), "selector.mjs".to_owned()],
    );
    let settings = CoreSettings::from_map(&on, workspace_root()).unwrap();
    assert_eq!(settings.model_selector(), Some(&expected));

    for (command, args) in [
        (Some("/usr/bin/env"), None),
        (None, Some("[]")),
        (Some("relative/selector"), Some("[]")),
        (Some("/usr/bin/env"), Some("not json")),
        (Some("/usr/bin/env"), Some(r#"{"a":1}"#)),
    ] {
        let mut values = valid_values();
        for (name, value) in [(COMMAND, command), (ARGS, args)] {
            if let Some(value) = value {
                values.insert(name.to_owned(), value.to_owned());
            }
        }
        let error = CoreSettings::from_map(&values, workspace_root()).unwrap_err();
        assert_eq!(
            error.code(),
            "invalid_configuration",
            "{command:?} {args:?}"
        );
    }
}
