//! Durable call arguments describe the pending compare-and-replace transition.
use crate::workspace::{MAX_FILE_BYTES, Workspace, error::Error};
use brainstone::runtime::DurableStore;
use brainstone::{Tool, ToolError, ToolInvocation, typed_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

const CRASH_EXIT_CODE: i32 = 73;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditFile {
    path: String,
    expected: String,
    replacement: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Receipt {
    path: String,
    bytes: usize,
}

pub(super) fn tool(
    workspace: Workspace,
    store: DurableStore,
    crash: bool,
) -> Result<impl Tool, ToolError> {
    typed_tool(
        "EditFile",
        "Replace an existing UTF-8 file only when its complete expected contents match. Edits are forbidden after successful checks.",
        move |args: EditFile, invocation: ToolInvocation| {
            let workspace = workspace.clone();
            let store = store.clone();
            async move {
                invocation.require_idempotency_key()?;
                super::guard::before_edit(&store, &invocation).await?;
                let receipt = replace(&workspace, args)?;
                if crash {
                    std::process::exit(CRASH_EXIT_CODE);
                }
                Ok::<_, ToolError>(receipt)
            }
        },
    )
}

fn replace(workspace: &Workspace, args: EditFile) -> Result<Receipt, ToolError> {
    if args.expected.len().max(args.replacement.len()) > MAX_FILE_BYTES {
        return Err(Error::FileTooLarge.into());
    }
    if args.expected == args.replacement {
        return Err(Error::NoChange.into());
    }
    let (path, current) = workspace.text(&args.path)?;
    if current == args.expected {
        atomic_replace(&path, args.replacement.as_bytes())?;
    } else if current != args.replacement {
        return Err(Error::FileConflict.into());
    }
    sync_replacement(&path)?;
    Ok(Receipt {
        path: args.path,
        bytes: args.replacement.len(),
    })
}

fn atomic_replace(path: &Path, contents: &[u8]) -> Result<(), Error> {
    let parent = path.parent().ok_or(Error::InvalidPath)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(fs::metadata(path)?.permissions())?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn sync_replacement(path: &Path) -> Result<(), Error> {
    // A retry may observe a rename whose writer exited before syncing its parent.
    fs::File::open(path)?.sync_all()?;
    fs::File::open(path.parent().ok_or(Error::InvalidPath)?)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
