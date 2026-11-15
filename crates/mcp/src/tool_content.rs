//! MCP content as the model reads it: text stays text, a decodable image
//! becomes model input named in the text, a file blob resource is kept for the
//! reply and named in the text, and any other binary block becomes a note
//! naming what it was and why it was left out.

use agent::{ToolError, ToolOutput};
use rmcp::model::{CallToolResult, ContentBlock, ResourceContents};

use crate::tool_content_note::{Binary, Omission};

/// The tool output for a completed call, or its error document.
pub(crate) fn output(result: CallToolResult) -> Result<ToolOutput, ToolError> {
    if result.is_error == Some(true) {
        let text = encode(&omitted(result))?;
        return Err(ToolError::new("mcp_tool_error", text));
    }
    let (result, mut output) = readable(result);
    output.text = encode(&result)?;
    Ok(output)
}

fn encode(result: &CallToolResult) -> Result<String, ToolError> {
    serde_json::to_string(result)
        .map_err(|error| ToolError::new("mcp_result_encode", error.to_string()))
}

/// A successful result: images the model can take, and files the reply can
/// attach, travel beside the text in an output whose text is unset.
pub(crate) fn readable(mut result: CallToolResult) -> (CallToolResult, ToolOutput) {
    let mut kept = ToolOutput::default();
    result.content = result
        .content
        .into_iter()
        .map(|block| read(block, &mut kept))
        .collect();
    (result, kept)
}

fn read(block: ContentBlock, kept: &mut ToolOutput) -> ContentBlock {
    if let ContentBlock::Image(image) = &block {
        return crate::tool_content_image::read(&image.data, &image.mime_type, &mut kept.images);
    }
    let Some(binary) = binary(&block) else {
        return block;
    };
    if binary.media_type.starts_with(VIDEO) {
        return binary.note(Omission::VideoUnsupported);
    }
    if matches!(block, ContentBlock::Resource(_)) {
        return crate::tool_content_file::read(&binary, &mut kept.files);
    }
    binary.note(Omission::Binary)
}

/// Every `video/*` blob is named as an unsupported video rather than as an
/// anonymous binary.
const VIDEO: &str = "video/";

/// A failed result reaches the model as text alone, so every binary is noted.
pub(crate) fn omitted(mut result: CallToolResult) -> CallToolResult {
    result.content = result
        .content
        .into_iter()
        .map(|block| omit(&block).unwrap_or(block))
        .collect();
    result
}

/// The note for a binary block; `None` for a block that is already text.
fn omit(block: &ContentBlock) -> Option<ContentBlock> {
    binary(block).map(|binary| binary.note(Omission::Binary))
}

fn binary(block: &ContentBlock) -> Option<Binary<'_>> {
    let (what, media_type, data) = match block {
        ContentBlock::Image(image) => ("image", image.mime_type.as_str(), &image.data),
        ContentBlock::Audio(audio) => ("audio", audio.mime_type.as_str(), &audio.data),
        ContentBlock::Resource(embedded) => match &embedded.resource {
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } => (
                uri.as_str(),
                mime_type
                    .as_deref()
                    .unwrap_or(crate::tool_content_note::UNKNOWN_MEDIA_TYPE),
                blob,
            ),
            _ => return None,
        },
        _ => return None,
    };
    Some(Binary {
        what,
        media_type,
        data,
    })
}
