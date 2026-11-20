#[test]
fn declared_model_window_tightens_compaction_without_changing_the_configured_limit() {
    let policy = crate::CompactionPolicy::new(200_000, "summarize").unwrap();
    let thread = agent::Thread::restore(
        "s",
        vec![
            agent::Message::User("old".repeat(300)),
            agent::Message::User("new".to_owned()),
        ],
    )
    .unwrap();
    assert!(!policy.reached_with_window(&thread, &[], None).unwrap());
    assert!(policy.reached_with_window(&thread, &[], Some(512)).unwrap());
    assert!(
        !policy
            .reached_with_window(&thread, &[], Some(100_000))
            .unwrap()
    );
    assert_eq!(policy.token_limit, 200_000);
}
