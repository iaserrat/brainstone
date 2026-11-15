use std::path::Path;

use agent::Registry;
use agent::{ToolCall, execute_tool};
use brainstone_skills::Catalog;

#[test]
fn initial_tool_catalog_discloses_metadata_but_not_instructions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let tools = Catalog::load(&root).registry(Registry::default());

    let definitions = tools.definitions();
    let skill = definitions
        .iter()
        .find(|definition| definition["name"] == "Skill")
        .expect("skill activation tool");
    let description = skill["description"].as_str().expect("description");

    assert!(description.contains("report-filing"));
    assert!(description.contains("Files finished reports"));
    assert!(!description.contains("# Report filing"));
    assert_eq!(
        skill["input_schema"]["properties"]["name"]["enum"],
        serde_json::json!(["report-filing"])
    );
    assert!(
        definitions
            .iter()
            .any(|definition| definition["name"] == "SkillResource")
    );
}

#[test]
fn an_empty_catalog_registers_no_skill_tools() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/absent");

    let tools = Catalog::load(&root).registry(Registry::default());

    assert_eq!(tools.definitions(), Vec::<serde_json::Value>::new());
}

#[tokio::test]
async fn activation_loads_the_complete_skill_but_only_lists_resources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let tools = Catalog::load(&root).registry(Registry::default());
    let call = ToolCall {
        id: "skill-1".to_owned(),
        name: "Skill".to_owned(),
        args: serde_json::json!({"name":"report-filing"}),
    };

    let result = execute_tool(&call, &tools, None).await;

    assert!(!result.is_error);
    assert!(
        result
            .content
            .contains("<skill_content name=\"report-filing\">")
    );
    assert!(result.content.contains("# Report filing"));
    assert!(result.content.contains("references/guide.md"));
    assert!(result.content.contains("scripts/extract.py"));
    assert!(!result.content.contains("REFERENCE CONTENT IS LAZY"));
    assert!(!result.content.contains("script stays lazy"));
    assert!(
        result
            .content
            .contains("Relative resource paths are resolved")
    );
}

#[tokio::test]
async fn a_resource_is_loaded_only_when_requested() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let tools = Catalog::load(&root).registry(Registry::default());
    let call = ToolCall {
        id: "resource-1".to_owned(),
        name: "SkillResource".to_owned(),
        args: serde_json::json!({
            "name":"report-filing",
            "path":"references/guide.md"
        }),
    };

    let result = execute_tool(&call, &tools, None).await;

    assert!(!result.is_error);
    let output: serde_json::Value = serde_json::from_str(&result.content).expect("JSON output");
    assert_eq!(output["skill"], "report-filing");
    assert_eq!(output["path"], "references/guide.md");
    assert_eq!(output["contents"], "REFERENCE CONTENT IS LAZY\n");
}

#[tokio::test]
async fn resource_paths_cannot_escape_the_skill_directory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let tools = Catalog::load(&root).registry(Registry::default());
    let call = ToolCall {
        id: "resource-escape".to_owned(),
        name: "SkillResource".to_owned(),
        args: serde_json::json!({"name":"report-filing","path":"../SKILL.md"}),
    };

    let result = execute_tool(&call, &tools, None).await;

    assert!(result.is_error);
    let output: serde_json::Value = serde_json::from_str(&result.content).expect("error JSON");
    assert_eq!(output["error"], "invalid_skill_resource_path");
}

#[tokio::test]
async fn unknown_skill_names_return_a_stable_error() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");
    let tools = Catalog::load(&root).registry(Registry::default());
    let call = ToolCall {
        id: "unknown-skill".to_owned(),
        name: "Skill".to_owned(),
        args: serde_json::json!({"name":"not-installed"}),
    };

    let result = execute_tool(&call, &tools, None).await;

    assert!(result.is_error);
    let output: serde_json::Value = serde_json::from_str(&result.content).expect("error JSON");
    assert_eq!(output["error"], "unknown_skill");
}
