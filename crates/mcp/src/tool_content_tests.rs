//! What the model reads of each MCP content block, one case per block kind.

use rmcp::model::{CallToolResult, ContentBlock, ResourceContents};

use crate::tests::PNG_1X1_BASE64;
use crate::tool_content::{omitted, readable};

fn texts(result: &CallToolResult) -> Vec<String> {
    result
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text(text) => text.text.clone(),
            other => panic!("binary reached the model as a block: {other:?}"),
        })
        .collect()
}

fn one(block: ContentBlock) -> (String, usize) {
    let (result, kept) = readable(CallToolResult::success(vec![block]));
    let count = kept.images.len() + kept.files.len();
    (texts(&result).remove(0), count)
}

/// An MP4 header as a sandbox server would send it: an embedded blob.
fn video_blob(bytes: &[u8], media_type: &str) -> ContentBlock {
    use base64::Engine as _;
    let blob = base64::engine::general_purpose::STANDARD.encode(bytes);
    ContentBlock::resource(
        ResourceContents::blob(blob, "sandbox://s1/run ☕.mp4").with_mime_type(media_type),
    )
}

const MP4: &[u8] = b"\0\0\0\x10ftypisom\0\0\0\0";

#[test]
fn a_decodable_image_becomes_model_input_named_by_its_reference() {
    let (result, kept) = readable(CallToolResult::success(vec![
        ContentBlock::text("before ☕"),
        ContentBlock::image(PNG_1X1_BASE64, "image/png"),
    ]));
    let images = kept.images;

    assert_eq!(images.len(), 1);
    let texts = texts(&result);
    assert_eq!(texts[0], "before ☕");
    let attachment: serde_json::Value = serde_json::from_str(&texts[1]).unwrap();
    assert_eq!(attachment, media::attachment(&images[0]));
}

/// Unpadded base64 and a data URL carry the same bytes as the canonical form.
#[test]
fn unpadded_and_data_url_images_reach_the_model() {
    let unpadded = PNG_1X1_BASE64.trim_end_matches('=');
    let data_url = format!("data:image/png;base64,{PNG_1X1_BASE64}");
    for data in [unpadded.to_owned(), data_url] {
        let (text, images) = one(ContentBlock::image(data, "image/png"));
        assert_eq!(images, 1, "{text}");
    }
}

#[test]
fn images_the_model_cannot_take_become_coded_notes() {
    for (block, note) in [
        (
            ContentBlock::image("R0lGODlhAQABAAAAACw=", "image/gif"),
            "[mcp_image_unsupported: image (image/gif, 14 bytes)",
        ),
        (
            ContentBlock::image("not base64!", "image/png"),
            "[mcp_image_invalid: image (image/png, ",
        ),
        (
            ContentBlock::image("iVBORw0KGgo=", "image/png"),
            "[mcp_image_invalid: image (image/png, 8 bytes)",
        ),
        (
            ContentBlock::image(PNG_1X1_BASE64, "image/jpeg"),
            "[mcp_image_invalid: image (image/jpeg, ",
        ),
    ] {
        let (text, images) = one(block);
        assert_eq!(images, 0, "{text}");
        assert!(text.starts_with(note), "{text}");
        assert!(text.ends_with("was not passed to the model]"), "{text}");
    }
}

#[test]
fn an_image_past_the_byte_cap_is_too_large() {
    let oversized = "A".repeat((media::MAX_IMAGE_BYTES + 3) / 3 * 4);

    let (text, images) = one(ContentBlock::image(oversized, "image/png"));

    assert_eq!(images, 0);
    assert!(
        text.starts_with("[mcp_image_too_large: image (image/png, "),
        "{text}"
    );
}

/// Each image block of one result is its own model input with its own name.
#[test]
fn every_image_block_of_a_result_gets_its_own_reference() {
    let second = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    let (result, kept) = readable(CallToolResult::success(vec![
        ContentBlock::image(PNG_1X1_BASE64, "image/png"),
        ContentBlock::image(second, "image/png"),
        ContentBlock::image(PNG_1X1_BASE64, "image/png"),
    ]));

    assert_eq!(kept.images.len(), 3);
    let references: Vec<String> = texts(&result)
        .iter()
        .map(|text| serde_json::from_str::<serde_json::Value>(text).unwrap())
        .map(|attachment| attachment["reference"].as_str().unwrap().to_owned())
        .collect();
    assert_ne!(references[0], references[1]);
    assert_eq!(references[0], references[2]);
}

/// A video reaches the model as a coded note and nothing is kept for the
/// reply, whatever container or size the server declares.
#[test]
fn every_video_becomes_a_coded_note() {
    let webm = [0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0, 0, 0, 0, 0];
    for (block, note) in [
        (
            video_blob(MP4, "video/mp4"),
            "[mcp_video_unsupported: sandbox://s1/run ☕.mp4 (video/mp4, 16 bytes)",
        ),
        (
            video_blob(&webm, "video/webm"),
            "[mcp_video_unsupported: sandbox://s1/run ☕.mp4 (video/webm, 12 bytes)",
        ),
    ] {
        let (text, kept) = one(block);
        assert_eq!(kept, 0, "{text}");
        assert!(text.starts_with(note), "{text}");
        assert!(
            text.ends_with("and cannot be shown in the reply]"),
            "{text}"
        );
    }
}

#[test]
fn audio_stays_a_binary_note() {
    let audio = ContentBlock::audio("AAEC", "audio/wav");

    let note = "[mcp_binary_omitted: audio (audio/wav, 3 bytes) is binary and was not passed to the model]";
    assert_eq!(one(audio), (note.to_owned(), 0));
}

fn blob(data: &str, uri: &str, media_type: Option<&str>) -> ContentBlock {
    let blob = ResourceContents::blob(data, uri);
    ContentBlock::resource(match media_type {
        Some(media_type) => blob.with_mime_type(media_type),
        None => blob,
    })
}

/// A fetched PDF, or any blob that is not a video, is kept whole for the
/// reply as a file named after its URI; the model reads only its document.
#[test]
fn a_blob_resource_is_kept_for_the_reply_as_a_file_named_by_its_uri() {
    let uri = "repo://o/r/contents/rapport%20%E2%98%95.pdf?ref=main#page=2";
    let (result, kept) = readable(CallToolResult::success(vec![
        blob("JVBERi0=", uri, Some("application/pdf")),
        blob("AAEC", "repo://chart.png", Some("image/png")),
        blob("AAEC", "sandbox://out/data", None),
    ]));

    let named: Vec<_> = kept
        .files
        .iter()
        .map(|f| (f.name(), f.media_type()))
        .collect();
    assert_eq!(
        named,
        [
            ("rapport ☕.pdf", "application/pdf"),
            ("chart.png", "image/png"),
            ("data", "application/octet-stream"),
        ]
    );
    assert_eq!(kept.files[0].bytes(), b"%PDF-");
    assert_eq!(kept.images, []);
    let mut expected = media::file_attachment(&kept.files[0]);
    expected["seen_by_model"] = false.into();
    let attachment: serde_json::Value = serde_json::from_str(&texts(&result)[0]).unwrap();
    assert_eq!(attachment, expected);
}

/// A percent sequence that does not decode to UTF-8 is kept as written.
#[test]
fn a_name_that_does_not_decode_keeps_its_uri_spelling() {
    let (_, kept) = readable(CallToolResult::success(vec![blob(
        "AAEC",
        "repo://x/100%25-%FF.bin",
        None,
    )]));

    assert_eq!(kept.files[0].name(), "100%25-%FF.bin");
}

#[test]
fn blobs_the_reply_cannot_attach_become_coded_notes() {
    let oversized = "A".repeat((media::MAX_FILE_BYTES + 3) / 3 * 4);
    for (block, note) in [
        (
            blob("not base64!", "repo://x/a.pdf", Some("application/pdf")),
            "[mcp_file_invalid: repo://x/a.pdf (application/pdf, ",
        ),
        (
            blob("AAEC", "repo://x/folder/", Some("application/pdf")),
            "[mcp_file_invalid: repo://x/folder/ (application/pdf, 3 bytes)",
        ),
        (
            blob(&oversized, "repo://x/big.zip", Some("application/zip")),
            "[mcp_file_too_large: repo://x/big.zip (application/zip, ",
        ),
    ] {
        let (text, kept) = one(block);
        assert_eq!(kept, 0, "{text}");
        assert!(text.starts_with(note), "{text}");
        assert!(
            text.ends_with("and cannot be attached to the reply]"),
            "{text}"
        );
    }
}

/// An error result reaches the model as text alone, so an image in it must
/// not claim to have been attached.
#[test]
fn an_error_result_notes_its_images_and_videos_instead_of_passing_them() {
    let result = omitted(CallToolResult::error(vec![
        ContentBlock::image(PNG_1X1_BASE64, "image/png"),
        video_blob(MP4, "video/mp4"),
    ]));

    let texts = texts(&result);
    assert!(
        texts[0].starts_with("[mcp_binary_omitted: image (image/png, "),
        "{}",
        texts[0]
    );
    assert!(
        texts[1].starts_with("[mcp_binary_omitted: sandbox://s1/run ☕.mp4 (video/mp4, "),
        "{}",
        texts[1]
    );
}
