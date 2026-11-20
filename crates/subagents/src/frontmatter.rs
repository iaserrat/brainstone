use std::path::Path;

use crate::{Error, raw::RawDefinition};

pub(crate) fn parse(path: &Path, text: &str) -> Result<(RawDefinition, String), Error> {
    let normalized = text.replace("\r\n", "\n");
    let content = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| invalid(path, "missing opening ---"))?;
    let end = content
        .find("\n---\n")
        .ok_or_else(|| invalid(path, "missing closing ---"))?;
    let yaml = &content[..end];
    let body = content[end + 5..].to_owned();
    let raw = serde_saphyr::from_str(yaml).map_err(|error| invalid(path, &error.to_string()))?;
    Ok((raw, body))
}

fn invalid(path: &Path, message: &str) -> Error {
    Error::InvalidFrontmatter {
        path: path.to_owned(),
        message: message.to_owned(),
    }
}
