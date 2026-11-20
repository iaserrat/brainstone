use super::*;

fn args() -> EditFile {
    EditFile {
        path: "lib.rs".into(),
        expected: "before ☕\n".into(),
        replacement: "after ☕\n".into(),
    }
}

#[test]
fn replacement_reconciles_replay_without_overwriting_conflicting_work() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("lib.rs"), &args().expected).unwrap();
    let workspace = Workspace::open(root.path()).unwrap();
    let first = replace(&workspace, args()).unwrap();
    let repeated = replace(&workspace, args()).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(
        fs::read_to_string(root.path().join("lib.rs")).unwrap(),
        args().replacement
    );
    fs::write(root.path().join("lib.rs"), "someone else's edit").unwrap();
    assert_eq!(
        replace(&workspace, args()).unwrap_err().code,
        "coding_file_conflict"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("lib.rs")).unwrap(),
        "someone else's edit"
    );
}

#[test]
fn no_op_and_oversized_edits_fail_before_writing() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("lib.rs"), &args().expected).unwrap();
    let workspace = Workspace::open(root.path()).unwrap();
    let mut same = args();
    same.replacement.clone_from(&same.expected);
    assert_eq!(
        replace(&workspace, same).unwrap_err().code,
        "coding_no_change"
    );
    let mut large = args();
    large.replacement = "a".repeat(MAX_FILE_BYTES + 1);
    assert_eq!(
        replace(&workspace, large).unwrap_err().code,
        "coding_file_too_large"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("lib.rs")).unwrap(),
        args().expected
    );
}
