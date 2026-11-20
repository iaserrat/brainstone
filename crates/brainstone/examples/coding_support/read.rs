use crate::workspace::{Workspace, error::Error};
use brainstone::{Tool, ToolError, typed_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;

const MAX_ENTRIES: usize = 64;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadFile {
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Contents {
    File { path: String, text: String },
    Directory { path: String, entries: Vec<String> },
}

pub(super) fn tool(workspace: Workspace) -> Result<impl Tool, ToolError> {
    typed_tool(
        "ReadFile",
        "Read a UTF-8 file or list a directory within the workspace; use . for the root.",
        move |args: ReadFile, _| {
            let workspace = workspace.clone();
            async move { read(&workspace, args) }
        },
    )
}

fn read(workspace: &Workspace, args: ReadFile) -> Result<Contents, ToolError> {
    let path = workspace.resolve(&args.path)?;
    if fs::metadata(&path).map_err(Error::from)?.is_dir() {
        return Ok(Contents::Directory {
            path: args.path,
            entries: entries(&path)?,
        });
    }
    let (_, text) = workspace.text(&args.path)?;
    Ok(Contents::File {
        path: args.path,
        text,
    })
}

fn entries(path: &std::path::Path) -> Result<Vec<String>, ToolError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(Error::from)? {
        let entry = entry.map_err(Error::from)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::InvalidPath)?;
        if name.starts_with('.') || name == "target" {
            continue;
        }
        entries.push(name);
        if entries.len() > MAX_ENTRIES {
            return Err(Error::DirectoryTooLarge.into());
        }
    }
    entries.sort();
    Ok(entries)
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
