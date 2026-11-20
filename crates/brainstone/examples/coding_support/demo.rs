//! A fixed fixture is created once; retries never reset edited source files.
use brainstone::agent::ToolCall;
use std::{fs, path::Path};

pub(crate) const BEFORE: &str = "pub fn add(a: i32, b: i32) -> i32 { a - b }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds_two_numbers() { assert_eq!(super::add(2, 3), 5); }\n}\n";
const MANIFEST: &str = "[package]\nname = \"brainstone-coding-demo\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n";
const LOCK: &str =
    "version = 4\n\n[[package]]\nname = \"brainstone-coding-demo\"\nversion = \"0.0.0\"\n";
const MARKER: &str = "brainstone-coding-demo-v1\n";

pub(crate) fn prepare(root: &Path) -> Result<(), super::Error> {
    if fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_symlink()) {
        return Err("Demo workspace root must not be a symlink".into());
    }
    fs::create_dir_all(root)?;
    let marker = root.join(".brainstone/demo");
    if marker.exists() {
        if fs::read_to_string(&marker)? == MARKER
            && fs::read_to_string(root.join("Cargo.toml"))? == MANIFEST
        {
            return Ok(());
        }
        return Err("Demo fixture identity changed; use a new empty workspace".into());
    }
    if fs::read_dir(root)?.next().is_some() {
        return Err("Demo needs a new or empty workspace".into());
    }
    fs::create_dir(root.join("src"))?;
    fs::create_dir(root.join(".brainstone"))?;
    fs::write(root.join("Cargo.toml"), MANIFEST)?;
    fs::write(root.join("Cargo.lock"), LOCK)?;
    fs::write(root.join("src/lib.rs"), BEFORE)?;
    fs::write(marker, MARKER)?;
    Ok(())
}
pub(crate) fn after() -> String {
    BEFORE.replace("a - b", "a + b")
}
pub(crate) fn edit_args() -> serde_json::Value {
    serde_json::json!({"path":"src/lib.rs","expected":BEFORE,"replacement":after()})
}
pub(crate) fn approves(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "EditFile" => call.args == edit_args(),
        "RunChecks" => call.args == serde_json::json!({}),
        _ => false,
    }
}

#[cfg(all(test, unix))]
#[path = "demo_tests.rs"]
mod tests;
