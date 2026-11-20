use agent::Image;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Every reference starts with this, which is what tells a reply's Markdown
/// image apart from an ordinary link.
pub const REFERENCE_PREFIX: &str = "image:";

/// Digest bytes kept: 64 bits names an image uniquely within a conversation
/// while staying short enough for the model to copy exactly.
const DIGEST_BYTES: usize = 8;

const HEX: &[u8; 16] = b"0123456789abcdef";

/// The stable name a reply uses for `image`: a digest of its bytes, so the
/// same image always has the same name in every build.
#[must_use]
pub fn reference(image: &Image) -> String {
    name(REFERENCE_PREFIX, &[image.bytes()])
}

/// `prefix` and the digest of `parts`, fed in order.
pub(crate) fn name(prefix: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut name = String::from(prefix);
    for byte in &digest[..DIGEST_BYTES] {
        name.push(char::from(HEX[usize::from(byte >> 4)]));
        name.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    name
}

/// What a tool's result says about an image it returned: what it is, and the
/// exact Markdown that shows it in the reply on a surface that can attach it.
#[must_use]
pub fn attachment(image: &Image) -> Value {
    let reference = reference(image);
    json!({
        "show_in_reply": show(&reference),
        "reference": reference,
        "media_type": image.media_type().as_str(),
        "width": image.width(),
        "height": image.height(),
    })
}

fn show(reference: &str) -> String {
    format!("![short description]({reference})")
}
