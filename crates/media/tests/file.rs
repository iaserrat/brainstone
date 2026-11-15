//! A file's reference is how a reply names a file a tool made or returned,
//! so a surface can attach it. It lives only as reply text, so it must be the
//! same in every build that will ever read that text back.

use agent::{File, Tool, ToolRequest};
use serde_json::{Value, json};

fn file(name: &str, media_type: &str) -> File {
    File::new(name, media_type, b"# A".to_vec()).expect("valid file")
}

#[test]
fn a_reference_is_a_pinned_digest_of_name_media_type_and_bytes() {
    // SHA-256 of "notes.md\0text/markdown\0# A"; pinning it is what keeps a
    // reply written before a deploy resolvable after it.
    assert_eq!(
        brainstone_media::file_reference(&file("notes.md", "text/markdown")),
        "file:d0e4695776620e2f"
    );
}

// Two files with the same bytes are still two attachments: the name is what
// the person saves, so it is part of what the reference names.
#[test]
fn the_same_bytes_under_another_name_or_type_is_another_file() {
    let first = brainstone_media::file_reference(&file("notes.md", "text/markdown"));

    assert_ne!(
        first,
        brainstone_media::file_reference(&file("notes.txt", "text/markdown"))
    );
    assert_ne!(
        first,
        brainstone_media::file_reference(&file("notes.md", "text/plain"))
    );
    assert!(first.starts_with(brainstone_media::FILE_REFERENCE_PREFIX));
}

#[test]
fn an_attachment_tells_the_model_how_to_attach_the_file() {
    assert_eq!(
        brainstone_media::file_attachment(&file("[draft] notes.md", "text/plain")),
        json!({
            "reference": "file:b13912068bc2ac8f",
            "name": "[draft] notes.md",
            "media_type": "text/plain",
            "bytes": 3,
            "show_in_reply": "[\\[draft\\] notes.md](file:b13912068bc2ac8f)",
        })
    );
}

async fn create(args: Value) -> Result<agent::ToolOutput, agent::ToolError> {
    let request = ToolRequest {
        args,
        idempotency_key: None,
        context: None,
    };
    brainstone_media::CreateFile.call(request).await
}

#[tokio::test]
async fn create_file_hands_the_reply_a_utf8_text_file() {
    let content = "# Résumé\n\nCafé ☕, naïve 日本語\n";

    let output = create(json!({"name": "résumé.md", "content": content}))
        .await
        .expect("created");

    let made = file_named("résumé.md", content);
    assert_eq!(output.files, vec![made.clone()]);
    let text: Value = serde_json::from_str(&output.text).expect("one JSON document");
    assert_eq!(text, brainstone_media::file_attachment(&made));
    assert_eq!(output.images, []);
}

fn file_named(name: &str, content: &str) -> File {
    File::new(name, "text/plain", content.as_bytes().to_vec()).expect("valid file")
}

#[tokio::test]
async fn create_file_refuses_what_it_cannot_make_with_a_stable_code() {
    let cases = [
        (json!({"name": "a.md"}), "invalid_file_arguments"),
        (
            json!({"name": "a.md", "content": "x", "extra": 1}),
            "invalid_file_arguments",
        ),
        (
            json!({"name": "../a.md", "content": "x"}),
            "invalid_file_name",
        ),
        (json!({"name": "a.md", "content": ""}), "empty_file"),
    ];
    for (args, code) in cases {
        let error = create(args.clone()).await.expect_err("refused");
        assert_eq!(error.code, code, "{args}");
    }
}

#[tokio::test]
async fn create_file_refuses_content_past_the_file_limit() {
    let content = "a".repeat(brainstone_media::MAX_FILE_BYTES + 1);

    let error = create(json!({"name": "a.txt", "content": content}))
        .await
        .expect_err("too large");

    assert_eq!(error.code, "file_too_large");
    let fits = "a".repeat(brainstone_media::MAX_FILE_BYTES);
    assert!(
        create(json!({"name": "a.txt", "content": fits}))
            .await
            .is_ok()
    );
}

/// A model that can produce bytes (a generated PNG, a zipped bundle) sends
/// them as base64 with their media type; text may name its own type too.
#[tokio::test]
async fn create_file_takes_base64_bytes_and_an_explicit_media_type() {
    let png = create(json!({
        "name": "pixel.png",
        "content": "iVBORw0KGgo=",
        "encoding": "base64",
        "media_type": "image/png",
    }))
    .await
    .expect("created");
    let markdown = create(json!({"name": "a.md", "content": "# A", "media_type": "text/markdown"}))
        .await
        .expect("created");
    let unnamed = create(json!({"name": "blob", "content": "AAEC", "encoding": "base64"}))
        .await
        .expect("created");

    assert_eq!(png.files[0].bytes(), b"\x89PNG\r\n\x1a\n");
    assert_eq!(png.files[0].media_type(), "image/png");
    assert_eq!(markdown.files[0].media_type(), "text/markdown");
    assert_eq!(unnamed.files[0].media_type(), "application/octet-stream");
    assert_eq!(unnamed.files[0].bytes(), [0, 1, 2]);
}

#[tokio::test]
async fn create_file_refuses_bad_bytes_or_types_with_a_stable_code() {
    let cases = [
        (
            json!({"name": "a", "content": "not base64!", "encoding": "base64"}),
            "invalid_file_content",
        ),
        (
            json!({"name": "a", "content": "x", "encoding": "utf-16"}),
            "invalid_file_arguments",
        ),
        (
            json!({"name": "a", "content": "x", "media_type": "markdown"}),
            "invalid_media_type",
        ),
    ];
    for (args, code) in cases {
        let error = create(args.clone()).await.expect_err("refused");
        assert_eq!(error.code, code, "{args}");
    }
}

#[tokio::test]
async fn create_file_measures_base64_by_the_bytes_it_decodes_to() {
    let encoded = "A".repeat((brainstone_media::MAX_FILE_BYTES + 3) / 3 * 4);

    let error = create(json!({"name": "a.bin", "content": encoded, "encoding": "base64"}))
        .await
        .expect_err("too large");

    assert_eq!(error.code, "file_too_large");
}
