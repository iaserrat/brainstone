use std::path::Path;

use brainstone_subagents::{Catalog, Effort, Model};

#[test]
fn loads_anthropic_definition_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid");
    let catalog = Catalog::load(&root).expect("valid catalog");
    let agent = catalog.get("code-reviewer").expect("definition");

    assert_eq!(agent.description(), "Reviews code after implementation");
    assert_eq!(
        agent.system_prompt(),
        "Review carefully.\nReturn only findings.\n"
    );
    assert_eq!(
        agent.tools(),
        Some(&["Read".to_owned(), "mcp__github__*".to_owned()][..])
    );
    assert_eq!(
        agent.disallowed_tools(),
        &["mcp__github__delete".to_owned()]
    );
    assert_eq!(agent.model(), &Model::Alias("sonnet".to_owned()));
    assert_eq!(agent.max_turns().unwrap().get(), Some(12));
    assert_eq!(agent.effort(), Some(Effort::High));
    assert_eq!(agent.background(), Some(true));
    assert_eq!(agent.mcp_servers(), &["github".to_owned()]);
}

#[test]
fn invalid_turn_limits_retain_their_existing_failure_codes() {
    for (fixture, code) in [
        ("zero-turns", "invalid_agent_definition"),
        ("invalid-turns", "invalid_agent_frontmatter"),
    ] {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        assert_eq!(Catalog::load(&root).unwrap_err().code(), code);
    }
}
