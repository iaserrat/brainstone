//! The tool that lets any agent hand the reply a file it wrote, on any
//! surface that attaches files.

use std::pin::Pin;

use agent::{File, Tool, ToolError, ToolOutput, ToolRequest};
use serde::Deserialize;
use serde_json::{Value, json};

#[path = "create_file_content.rs"]
mod content;
use content::Encoding;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    name: String,
    content: String,
    #[serde(default)]
    encoding: Encoding,
    media_type: Option<String>,
}

/// Makes a file from the model's own content for the reply to attach. The
/// runtime stores it under the conversation; the model reads its reference
/// and the Markdown link that attaches it.
#[derive(Debug)]
pub struct CreateFile;

impl Tool for CreateFile {
    fn name(&self) -> &'static str {
        "CreateFile"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Create a file, such as a Markdown document, CSV, JSON, source code, or SVG, that your reply attaches as a downloadable file. Use it when the person asks for a file or document, or for content they will save or reuse rather than read in the message. The result's `show_in_reply` is the exact Markdown link that attaches the file: write it in your reply where the file belongs, and do not repeat the file's content there. Calling it again with other content makes another file.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "The file name with its extension, such as `report.md`: one name, no folders, up to 255 bytes."},
                    "content": {"type": "string", "description": "The complete file content: text, or base64 when `encoding` is `base64`."},
                    "encoding": {"type": "string", "enum": ["utf-8", "base64"], "description": "`utf-8` (default) for text; `base64` for binary bytes."},
                    "media_type": {"type": "string", "description": "The content's media type, such as `text/markdown` or `image/png`. Defaults to `text/plain` for text and `application/octet-stream` for base64."}
                },
                "required": ["name", "content"],
                "additionalProperties": false
            }
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { create(&request.args) })
    }
}

fn create(args: &Value) -> Result<ToolOutput, ToolError> {
    let args = Args::deserialize(args)
        .map_err(|error| ToolError::new("invalid_file_arguments", error.to_string()))?;
    let bytes = args.encoding.decode(args.content)?;
    let media_type = args
        .media_type
        .as_deref()
        .unwrap_or_else(|| args.encoding.media_type());
    let file = File::new(args.name, media_type, bytes)
        .map_err(|error| ToolError::new(error.code(), error.to_string()))?;
    Ok(ToolOutput {
        text: crate::file_attachment(&file).to_string(),
        files: vec![file],
        ..ToolOutput::default()
    })
}
