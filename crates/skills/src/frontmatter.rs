use std::path::Path;

use crate::{Diagnostic, raw::RawSkill};

pub(crate) fn parse(path: &Path, text: &str) -> Result<RawSkill, Diagnostic> {
    let normalized = text.replace("\r\n", "\n");
    let content = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| Diagnostic::invalid(path, "missing opening ---"))?;
    let end = content
        .find("\n---")
        .ok_or_else(|| Diagnostic::invalid(path, "missing closing ---"))?;
    decode(path, &content[..end])
}

fn decode(path: &Path, yaml: &str) -> Result<RawSkill, Diagnostic> {
    let original = match serde_saphyr::from_str(yaml) {
        Ok(raw) => return Ok(raw),
        Err(error) => error,
    };
    let Some(repaired) = repair(yaml) else {
        return Err(Diagnostic::invalid(path, original.to_string()));
    };
    serde_saphyr::from_str(&repaired).map_err(|_| Diagnostic::invalid(path, original.to_string()))
}

fn repair(yaml: &str) -> Option<String> {
    let mut changed = false;
    let lines = yaml
        .lines()
        .map(|line| repair_line(line, &mut changed))
        .collect::<Vec<_>>();
    changed.then(|| lines.join("\n"))
}

fn repair_line(line: &str, changed: &mut bool) -> String {
    let Some((key, value)) = line.split_once(':') else {
        return line.to_owned();
    };
    let scalar = value.trim();
    let quoted = matches!(
        scalar.chars().next(),
        Some('\'' | '"' | '|' | '>' | '[' | '{')
    );
    if line.starts_with(char::is_whitespace)
        || scalar.is_empty()
        || quoted
        || !scalar.contains(": ")
    {
        return line.to_owned();
    }
    *changed = true;
    format!("{key}: '{}'", scalar.replace('\'', "''"))
}
