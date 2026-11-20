use agent::File;
use serde_json::{Value, json};

use crate::reference::name;

/// What a file's reference starts with. The reply attaches it with a
/// Markdown link, since a file is not shown inline.
pub const FILE_REFERENCE_PREFIX: &str = "file:";

/// The largest file a tool may hand a reply. Like a video it is held whole in
/// memory and in one database row.
pub const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;

/// Separates the digested parts. Neither a name nor a media type can hold it.
const SEPARATOR: &[u8] = b"\0";

/// The stable name a reply uses for `file`: a digest of its name, media
/// type, and bytes, so the same bytes saved under two names are two files.
#[must_use]
pub fn file_reference(file: &File) -> String {
    let parts = [
        file.name().as_bytes(),
        SEPARATOR,
        file.media_type().as_bytes(),
        SEPARATOR,
        file.bytes(),
    ];
    name(FILE_REFERENCE_PREFIX, &parts)
}

/// What a tool's result says about a file it made or returned, and the exact
/// Markdown link that attaches it to the reply.
#[must_use]
pub fn file_attachment(file: &File) -> Value {
    let reference = file_reference(file);
    json!({
        "show_in_reply": format!("[{}]({reference})", link_text(file.name())),
        "reference": reference,
        "name": file.name(),
        "media_type": file.media_type(),
        "bytes": file.bytes().len(),
    })
}

/// The name as link text: a bracket or backslash in it would end the text
/// early, so each is escaped.
fn link_text(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    for character in name.chars() {
        if matches!(character, '[' | ']' | '\\') {
            text.push('\\');
        }
        text.push(character);
    }
    text
}
