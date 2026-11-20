//! Reject unsupported roots before fixture setup can write through them.
use super::*;
use std::os::unix::fs::symlink;

#[test]
fn a_symlink_root_is_rejected_without_modifying_its_target() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = directory.path().join("linked-workspace");
    symlink(&target, &link).unwrap();
    let result = prepare(&link);
    assert_eq!(
        (result.is_err(), fs::read_dir(target).unwrap().count()),
        (true, 0)
    );
}
