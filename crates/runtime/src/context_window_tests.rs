//! Contract: the policy limit and a declared window are in tokens, and the
//! size estimate (characters) is converted once, at the comparison.

fn thread(chars: usize) -> agent::Thread {
    agent::Thread::restore("s", vec![agent::Message::User("x".repeat(chars))]).unwrap()
}

#[test]
fn the_configured_limit_is_in_tokens_not_characters() {
    let policy = crate::CompactionPolicy::new(500, "summarize").unwrap();
    // 1200 characters is 400 tokens at the estimate's three characters per token.
    assert!(
        !policy
            .reached_with_window(&thread(1_200), &[], None)
            .unwrap()
    );
    assert!(
        policy
            .reached_with_window(&thread(1_800), &[], None)
            .unwrap()
    );
}

#[test]
fn a_declared_window_tightens_compaction_without_changing_the_configured_limit() {
    let policy = crate::CompactionPolicy::new(200_000, "summarize").unwrap();
    // 1200 characters, 400 tokens: under a 256-token window's 90% share (230).
    assert!(
        policy
            .reached_with_window(&thread(1_200), &[], Some(256))
            .unwrap()
    );
    assert!(
        !policy
            .reached_with_window(&thread(1_200), &[], Some(100_000))
            .unwrap()
    );
    assert_eq!(policy.token_limit, 200_000);
}

#[test]
fn a_declared_window_never_raises_the_configured_limit() {
    let policy = crate::CompactionPolicy::new(300, "summarize").unwrap();
    // A million-token window cannot talk the configured 300-token limit up.
    assert!(
        policy
            .reached_with_window(&thread(1_200), &[], Some(1_000_000))
            .unwrap()
    );
}
