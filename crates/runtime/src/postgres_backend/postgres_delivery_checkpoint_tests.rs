use super::terminal_checkpoint;

#[test]
fn reconciliation_is_owed_only_by_the_final_terminal_checkpoint() {
    assert!(terminal_checkpoint(true, 4, Some(3)));
    assert!(!terminal_checkpoint(false, 4, Some(3)));
    assert!(!terminal_checkpoint(true, 4, Some(2)));
    assert!(!terminal_checkpoint(true, 4, None));
}
