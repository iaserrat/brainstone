//! Tool-name patterns: `mcp__*` (every MCP tool), `<prefix>__*`, `mcp__<server>` (that
//! server's tools), or an exact name; a `(...)` suffix is ignored.

pub(crate) fn selected(name: &str, allow: Option<&[String]>, deny: &[String]) -> bool {
    !deny.iter().any(|pattern| matches(pattern, name))
        && allow.is_none_or(|patterns| patterns.iter().any(|pattern| matches(pattern, name)))
}

fn matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.split_once('(').map_or(pattern, |(base, _)| base);
    if pattern == "mcp__*" {
        return name.starts_with("mcp__");
    }
    if let Some(prefix) = pattern.strip_suffix("__*") {
        return name.starts_with(&format!("{prefix}__"));
    }
    if pattern.starts_with("mcp__") && pattern.matches("__").count() == 1 {
        return name.starts_with(&format!("{pattern}__"));
    }
    pattern == name
}
