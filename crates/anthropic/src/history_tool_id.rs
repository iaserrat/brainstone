use std::borrow::Cow;

/// `id` in the alphabet Messages accepts, `^[a-zA-Z0-9_-]+$`, one-to-one: `-`
/// becomes `--`, any other byte `-` and two hex digits, and an empty id `-`.
/// Distinct ids stay distinct, and an id with neither passes unchanged.
pub(super) fn tool_id(id: &str) -> Cow<'_, str> {
    if id.is_empty() {
        return Cow::Borrowed("-");
    }
    if id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Cow::Borrowed(id);
    }
    Cow::Owned(id.bytes().map(escaped).collect())
}

fn escaped(byte: u8) -> String {
    match byte {
        b'-' => "--".to_owned(),
        _ if byte.is_ascii_alphanumeric() || byte == b'_' => char::from(byte).to_string(),
        _ => format!("-{byte:02x}"),
    }
}
