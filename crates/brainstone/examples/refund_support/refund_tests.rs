//! A real subprocess exits after the effect, then reopens the same records.
use std::process::{Command, Output};

#[tokio::test]
async fn interrupted_simulated_refund_reuses_the_effect_after_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("refund.db").display());
    let crash = child(&url, true);
    assert_eq!(crash.status.code(), Some(73), "{:?}", crash);
    let resumed = child(&url, false);
    assert!(resumed.status.success(), "{:?}", resumed);
    assert!(String::from_utf8_lossy(&resumed.stdout).contains("Completed"));
    let repeated = child(&url, false);
    assert!(repeated.status.success(), "{:?}", repeated);
    let pool = super::ledger::open(&url).await.unwrap();
    let effects: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT retry_key,order_id,amount_cents FROM demo_refunds")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(effects.len(), 1);
    assert!(effects[0].0.starts_with("operation/"));
    assert_eq!((&effects[0].1, effects[0].2), (&"BS-1001".to_owned(), 500));
    pool.close().await;
}

fn child(url: &str, crash: bool) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "tests::worker", "--nocapture"])
        .env("BRAINSTONE_REFUND_TEST_URL", url)
        .env(
            "BRAINSTONE_REFUND_TEST_CRASH",
            if crash { "yes" } else { "no" },
        )
        .output()
        .unwrap()
}

#[tokio::test]
#[ignore = "subprocess fixture"]
async fn worker() {
    let url = std::env::var("BRAINSTONE_REFUND_TEST_URL").unwrap();
    let crash = std::env::var("BRAINSTONE_REFUND_TEST_CRASH").unwrap() == "yes";
    super::execute(super::input::Options { url, crash })
        .await
        .unwrap();
}
