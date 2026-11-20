//! A retry returns the original simulated refund and rejects changed arguments.
use super::*;

#[tokio::test]
async fn duplicate_refunds_keep_one_effect_and_conflicting_input_fails() {
    let pool = open("sqlite::memory:").await.unwrap();
    let first = record(&pool, "same-call", "BS-1001", 500).await.unwrap();
    let second = record(&pool, "same-call", "BS-1001", 500).await.unwrap();
    assert_eq!(first, second);
    assert!(matches!(
        record(&pool, "same-call", "BS-1001", 600).await,
        Err(RefundError::Conflict)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM demo_refunds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}

#[test]
fn refund_errors_expose_stable_tool_codes() {
    for (error, code) in [
        (RefundError::Conflict, "refund_conflict"),
        (
            RefundError::Storage(sqlx::Error::RowNotFound),
            "refund_storage_failed",
        ),
    ] {
        assert_eq!(brainstone::ToolError::from(error).code, code);
    }
}
