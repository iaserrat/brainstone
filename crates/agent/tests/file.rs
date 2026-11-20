//! A file a tool hands the reply: only a name a person can save as given,
//! a plain media type, and bytes; and it never enters the tool result.

use std::pin::Pin;
use std::sync::Arc;

use brainstone_agent::{File, Registry, Tool, ToolCall, ToolError, ToolOutput, ToolRequest};
use serde_json::{Value, json};

#[test]
fn a_file_keeps_its_unicode_name_and_its_media_type_essence() {
    let file = File::new(
        "Résumé ☕ 2026.md",
        " Text/Markdown; charset=UTF-8",
        b"# Hi".to_vec(),
    )
    .expect("valid file");

    assert_eq!(file.name(), "Résumé ☕ 2026.md");
    assert_eq!(file.media_type(), "text/markdown");
    assert_eq!(file.bytes(), b"# Hi");
}

// Each name here would be renamed, refused, or turned into a path by the
// surface that saves it, so the file would not arrive as the tool named it.
#[test]
fn a_name_a_person_could_not_save_as_given_is_refused() {
    let long = "a".repeat(brainstone_agent::MAX_FILE_NAME_BYTES + 1);
    for name in [
        "",
        " ",
        " notes.md",
        "notes.md ",
        ".",
        "..",
        "a/b.md",
        "a\\b.md",
        "a\nb.md",
        "a\0b",
        &long,
    ] {
        let error = File::new(name, "text/plain", vec![1]).expect_err(name);
        assert_eq!(error.code(), "invalid_file_name", "{name:?}");
    }
    let longest = "é".repeat(brainstone_agent::MAX_FILE_NAME_BYTES / 2);
    assert!(File::new(longest, "text/plain", vec![1]).is_ok());
}

#[test]
fn a_media_type_that_is_not_type_slash_subtype_is_refused() {
    for media_type in [
        "",
        "text",
        "text/",
        "/plain",
        "text/pl ain",
        "text/plain/x",
        "téxt/plain",
    ] {
        let error = File::new("a.txt", media_type, vec![1]).expect_err(media_type);
        assert_eq!(error.code(), "invalid_media_type", "{media_type:?}");
    }
}

#[test]
fn an_empty_file_is_refused() {
    let error = File::new("a.txt", "text/plain", Vec::new()).expect_err("empty");

    assert_eq!(error.code(), "empty_file");
}

struct Make;

impl Tool for Make {
    fn name(&self) -> &'static str {
        "Make"
    }

    fn definition(&self) -> Value {
        json!({"name": "Make", "description": "", "input_schema": {"type": "object"}})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async {
            Ok(ToolOutput {
                text: "made".to_owned(),
                files: vec![File::new("a.md", "text/markdown", b"# A".to_vec()).unwrap()],
                ..ToolOutput::default()
            })
        })
    }
}

#[tokio::test]
async fn a_file_travels_beside_the_result_and_never_in_it() {
    let tools = Registry::new(vec![Arc::new(Make)]);
    let call = ToolCall {
        id: "call-1".to_owned(),
        name: "Make".to_owned(),
        args: json!({}),
    };

    let (result, files) =
        brainstone_agent::execute_tool_with_media(&call, &tools, None, None).await;

    assert_eq!(result.content, "made");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name(), "a.md");
    let stored = serde_json::to_value(&result).expect("serializes");
    assert_eq!(
        stored,
        json!({"id": "call-1", "name": "Make", "content": "made", "is_error": false})
    );
}
