//! An MCP blob resource that is not a video: a fetched PDF, a generated
//! spreadsheet, an archive. The model cannot read it as text, so it is kept
//! whole for the reply as a file named after its URI.

use agent::File;
use base64::Engine as _;
use rmcp::model::ContentBlock;

use crate::tool_content_image::{BASE64, decoded_len, payload};
use crate::tool_content_note::{Binary, Omission};

/// The block that replaces a blob: its attachment document when the file
/// joins `files`, or the note saying why it did not.
pub(crate) fn read(binary: &Binary<'_>, files: &mut Vec<File>) -> ContentBlock {
    match decode(binary) {
        Ok(file) => {
            let mut attachment = media::file_attachment(&file);
            attachment["seen_by_model"] = false.into();
            files.push(file);
            ContentBlock::text(attachment.to_string())
        }
        Err(omission) => binary.note(omission),
    }
}

fn decode(binary: &Binary<'_>) -> Result<File, Omission> {
    let data = payload(binary.data);
    // Refuse an oversized file before allocating its bytes.
    if decoded_len(data) > media::MAX_FILE_BYTES {
        return Err(Omission::FileTooLarge);
    }
    let bytes = BASE64.decode(data).map_err(|_| Omission::FileInvalid)?;
    File::new(name(binary.what), binary.media_type, bytes).map_err(|_| Omission::FileInvalid)
}

/// The URI's last path segment, percent-decoded when that yields UTF-8, so a
/// `file:///tmp/my%20report.pdf` arrives as `my report.pdf`.
fn name(uri: &str) -> String {
    let path = uri.split(['?', '#']).next().unwrap_or_default();
    let segment = path.rsplit('/').next().unwrap_or_default();
    String::from_utf8(percent_decoded(segment)).unwrap_or_else(|_| segment.to_owned())
}

fn percent_decoded(segment: &str) -> Vec<u8> {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = bytes.get(index + 1..index + 3).and_then(hex_byte);
        match (bytes[index], escaped) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    decoded
}

fn hex_byte(pair: &[u8]) -> Option<u8> {
    let text = std::str::from_utf8(pair).ok()?;
    u8::from_str_radix(text, 16).ok()
}
