use super::*;

#[test]
fn directories_list_sorted_visible_entries_and_reject_overflow() {
    let root = tempfile::tempdir().unwrap();
    for name in ["z.rs", "a.rs", ".secret", "target"] {
        fs::write(root.path().join(name), "").unwrap();
    }
    let workspace = Workspace::open(root.path()).unwrap();
    let Contents::Directory { entries, .. } =
        read(&workspace, ReadFile { path: ".".into() }).unwrap()
    else {
        panic!("directory")
    };
    assert_eq!(entries, ["a.rs", "z.rs"]);
    for index in 0..MAX_ENTRIES {
        fs::write(root.path().join(format!("{index}.rs")), "").unwrap();
    }
    assert_eq!(
        read(&workspace, ReadFile { path: ".".into() })
            .unwrap_err()
            .code,
        "coding_directory_too_large"
    );
}
