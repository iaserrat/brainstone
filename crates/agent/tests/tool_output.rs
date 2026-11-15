//! The durable shape of a tool result: binaries survive the journal, and a
//! result recorded before attachments existed reads back exactly as written.

use brainstone_agent::{Image, ImageMediaType, ToolResult};
use serde_json::json;

#[test]
fn a_tool_result_round_trips_its_binaries() {
    let result = ToolResult {
        id: "call-1".to_owned(),
        name: "ReadThreadFile".to_owned(),
        content: r#"{"outcome":"file_read","name":"reçu ☕.png"}"#.to_owned(),
        is_error: false,
        images: vec![Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).expect("valid image")],
    };

    let stored = serde_json::to_value(&result).expect("serializes");
    let restored: ToolResult = serde_json::from_value(stored).expect("round trips");

    assert_eq!(restored, result);
}

#[test]
fn a_text_only_result_keeps_its_pre_attachment_shape() {
    let legacy = json!({"id": "call-1", "name": "Read", "content": "done", "is_error": false});

    let restored: ToolResult = serde_json::from_value(legacy.clone()).expect("legacy reads");

    assert_eq!(restored.images, []);
    assert_eq!(serde_json::to_value(&restored).expect("serializes"), legacy);
}
