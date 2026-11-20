use std::collections::BTreeMap;
use std::path::Path;

use crate::config_build;

#[test]
fn rejects_transport_options_from_the_other_transport() {
    let input = r#"
        [mcp_servers.mixed]
        command = "server"
        bearer_token_env = "TOKEN"
    "#;
    let error = config_build::build(input, Path::new("."), &environment())
        .expect_err("mixed transport must fail");
    assert_eq!(error.code(), "mcp_configuration");
    assert!(error.to_string().contains("stdio"));
}

#[test]
fn rejects_an_invalid_http_url_before_startup() {
    for url in ["not a URL", "file:///tmp/server.sock"] {
        let input = format!("[mcp_servers.bad]\nurl = '{url}'");
        let error = config_build::build(&input, Path::new("."), &BTreeMap::new())
            .expect_err("invalid URL must fail");
        assert_eq!(error.code(), "mcp_configuration");
    }
}

fn environment() -> BTreeMap<String, String> {
    BTreeMap::from([("TOKEN".into(), "secret".into())])
}

#[test]
fn restrictive_access_is_declarative_and_unknown_requirements_are_rejected() {
    let input = "[mcp_servers.personal]\ncommand = 'server'\naccess = { authenticated_actor = true, private_conversation = true }";
    let config = config_build::build(input, Path::new("."), &BTreeMap::new()).unwrap();
    assert!(config.servers()[0].access.check(None).is_err());
    let invalid = input.replace("private_conversation", "private_conversat0n");
    assert_eq!(
        config_build::build(&invalid, Path::new("."), &BTreeMap::new())
            .err()
            .unwrap()
            .code(),
        "mcp_configuration"
    );
}
