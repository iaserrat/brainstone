//! Workspace paths stay beneath a trusted root and reads have fixed bounds.
use super::*;
use std::fs;

#[test]
fn paths_reject_traversal_absolute_and_protected_components() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("hello.rs"), "reçu ☕").unwrap();
    let workspace = Workspace::open(root.path()).unwrap();
    assert_eq!(workspace.text("hello.rs").unwrap().1, "reçu ☕");
    for path in [
        "../hello.rs",
        "/etc/passwd",
        ".git/config",
        ".brainstone/state.db",
        "target/x",
        "a/../hello.rs",
        "",
    ] {
        assert_eq!(
            workspace.resolve(path).unwrap_err().code,
            "coding_invalid_path"
        );
    }
}

#[test]
fn text_reads_reject_invalid_utf8_and_oversized_content() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("invalid.rs"), [0xff]).unwrap();
    fs::write(root.path().join("large.rs"), vec![b'a'; MAX_FILE_BYTES + 1]).unwrap();
    let workspace = Workspace::open(root.path()).unwrap();
    assert_eq!(
        workspace.text("invalid.rs").unwrap_err().code,
        "coding_invalid_utf8"
    );
    assert_eq!(
        workspace.text("large.rs").unwrap_err().code,
        "coding_file_too_large"
    );
}

#[cfg(unix)]
#[test]
fn paths_reject_symlinks_even_when_the_target_is_inside_the_workspace() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("hello.rs"), "hello").unwrap();
    std::os::unix::fs::symlink("hello.rs", root.path().join("link.rs")).unwrap();
    let workspace = Workspace::open(root.path()).unwrap();
    assert_eq!(
        workspace.resolve("link.rs").unwrap_err().code,
        "coding_invalid_path"
    );
}
