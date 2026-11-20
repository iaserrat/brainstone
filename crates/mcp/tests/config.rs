//! Table-style configuration assertions intentionally remain one flow.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::path::Path;

use brainstone_mcp::{Config, Transport};

#[test]
fn loads_transports_and_resolves_only_named_environment_values() {
    let environment = BTreeMap::from([
        ("MCP_GITHUB_TOKEN".into(), "secret-token".into()),
        ("MCP_CHILD_SECRET".into(), "child-secret".into()),
        ("UNRELATED_SECRET".into(), "must-not-reach-child".into()),
        ("PATH".into(), "/usr/bin".into()),
    ]);
    let config = Config::load(&fixtures(), &environment).expect("valid MCP config");

    assert_eq!(config.servers().len(), 2);
    assert_eq!(config.servers()[0].name(), "github");
    assert!(config.servers()[0].required());
    assert!(matches!(
        config.servers()[0].transport(),
        Transport::Http { .. }
    ));
    assert_eq!(config.servers()[1].name(), "local");
    assert!(!config.servers()[1].required());
    assert!(matches!(
        config.servers()[1].transport(),
        Transport::Stdio { .. }
    ));
    let Transport::Stdio { env, .. } = config.servers()[1].transport() else {
        unreachable!();
    };
    assert_eq!(
        env.get("CHILD_SECRET").map(String::as_str),
        Some("child-secret")
    );
    assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
    assert!(!env.contains_key("UNRELATED_SECRET"));
}

#[test]
fn missing_referenced_environment_value_is_a_stable_configuration_error() {
    let error =
        Config::load(&fixtures(), &BTreeMap::new()).expect_err("missing environment must fail");
    assert_eq!(error.code(), "mcp_configuration");
    assert!(error.to_string().contains("MCP_GITHUB_TOKEN"));
}

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
