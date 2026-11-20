//! File-backed durability contracts; no external database is required.
#![allow(clippy::too_many_lines)] // Crash boundaries and table-driven contract scenarios.
use std::time::Duration;

use agent::{Event, Message, Outcome};
use brainstone_runtime::{Address, Prompt, SqliteStore, Store};

#[tokio::test]
async fn accepted_work_and_journal_survive_reopen_and_fence_the_crashed_writer() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("runtime.db").display());
    let store = SqliteStore::connect(&url).await.unwrap();
    let prompt = Prompt::new(
        Address::new("test", "session", "lane").unwrap(),
        "r1",
        "s",
        "héllo ☕",
    )
    .unwrap();
    let accepted = store.accept(&prompt).await.unwrap();
    let first = store
        .claim_operation(accepted.operation.id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let message = Message::User("durable steering".to_owned());
    store
        .record(
            &first.lease,
            &Event::Appended {
                message: message.clone(),
            },
        )
        .await
        .unwrap();
    store.heartbeat(&first.lease, Duration::ZERO).await.unwrap();
    store.close().await;
    let store = SqliteStore::connect(&url).await.unwrap();
    assert!(store.accept(&prompt).await.unwrap().duplicate);
    let recovered = store
        .claim_operation(accepted.operation.id, "recovery", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.thread.messages(),
        &[Message::User("héllo ☕".to_owned()), message]
    );
    assert_eq!(
        store
            .record(&first.lease, &Event::RunStarted)
            .await
            .unwrap_err()
            .code(),
        "lease_lost"
    );
    store
        .settle(
            &recovered.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn independent_pools_serialize_migrations_admission_and_session_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}",
        directory.path().join("concurrent.db").display()
    );
    let (first, second) = tokio::join!(SqliteStore::connect(&url), SqliteStore::connect(&url));
    let (first, second) = (first.unwrap(), second.unwrap());
    let prompt = Prompt::new(
        Address::new("test", "same-session", "lane").unwrap(),
        "one",
        "s",
        "go",
    )
    .unwrap();
    let (one, two) = tokio::join!(first.accept(&prompt), second.accept(&prompt));
    let (one, two) = (one.unwrap(), two.unwrap());
    assert_eq!(one.operation.id, two.operation.id);
    assert_ne!(one.duplicate, two.duplicate);
    let id = one.operation.id;
    let owned = first
        .claim_operation(id, "first", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert!(
        second
            .claim_operation(id, "second", Duration::from_secs(30))
            .await
            .unwrap()
            .is_none()
    );
    first.heartbeat(&owned.lease, Duration::ZERO).await.unwrap();
    let reclaimed = second
        .claim_operation(id, "second", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .heartbeat(&owned.lease, Duration::from_secs(30))
            .await
            .unwrap_err()
            .code(),
        "lease_lost"
    );
    second
        .settle(
            &reclaimed.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory.path().join("concurrent.db"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn unreadable_records_and_future_migrations_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}",
        directory.path().join("corruption.db").display()
    );
    let store = SqliteStore::connect(&url).await.unwrap();
    let prompt = Prompt::new(
        Address::new("test", "corrupt", "lane").unwrap(),
        "one",
        "s",
        "go",
    )
    .unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
    sqlx::query("INSERT INTO operation_events(operation_id,sequence,schema_version,payload) VALUES(?1,0,999,'{}')").bind(id.get()).execute(&pool).await.unwrap();
    assert_eq!(
        store
            .claim_operation(id, "worker", Duration::from_secs(30))
            .await
            .unwrap_err()
            .code(),
        "corrupt_record"
    );
    assert_eq!(
        store.operation(id).await.unwrap().unwrap().state,
        brainstone_runtime::OperationState::Accepted
    );
    sqlx::query("DELETE FROM operation_events")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE lanes SET execution_profile='{}'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .claim_operation(id, "worker", Duration::from_secs(30))
            .await
            .unwrap_err()
            .code(),
        "corrupt_record"
    );
    store.close().await;
    sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(999,'future',1,X'00',0)").execute(&pool).await.unwrap();
    pool.close().await;
    assert!(SqliteStore::connect(&url).await.is_err());
}

#[tokio::test]
async fn process_exit_after_commit_preserves_the_accepted_operation_and_journal() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("crash.db").display());
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "sqlite_crash_child", "--ignored"])
        .env("BRAINSTONE_SQLITE_CRASH_URL", &url)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    let store = SqliteStore::connect(&url).await.unwrap();
    let accepted = store.accept(&crash_prompt()).await.unwrap();
    assert!(accepted.duplicate);
    let reclaimed = store
        .claim_operation(accepted.operation.id, "recovered", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reclaimed.thread.messages(),
        &[
            Message::User("durable ☕".to_owned()),
            Message::User("committed".to_owned())
        ]
    );
    store
        .settle(
            &reclaimed.lease,
            &Outcome::Completed {
                text: "recovered".to_owned(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "subprocess crash fixture"]
async fn sqlite_crash_child() {
    let url = std::env::var("BRAINSTONE_SQLITE_CRASH_URL").unwrap();
    let store = SqliteStore::connect(&url).await.unwrap();
    let id = store.accept(&crash_prompt()).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .record(
            &claimed.lease,
            &Event::Appended {
                message: Message::User("committed".to_owned()),
            },
        )
        .await
        .unwrap();
    // Expiry is controlled; exit still leaves the pool and WAL uncheckpointed.
    store
        .heartbeat(&claimed.lease, Duration::ZERO)
        .await
        .unwrap();
    // Exit without dropping the pool or checkpointing its WAL.
    std::process::exit(73);
}

fn crash_prompt() -> Prompt {
    Prompt::new(
        Address::new("test", "process-crash", "lane").unwrap(),
        "one",
        "s",
        "durable ☕",
    )
    .unwrap()
}

#[tokio::test]
async fn in_memory_databases_cannot_claim_durability() {
    for url in [
        "sqlite::memory:",
        "sqlite://:memory:",
        "sqlite://named?mode=memory",
    ] {
        assert_eq!(
            SqliteStore::connect(url).await.unwrap_err().code(),
            "durability_not_guaranteed"
        );
    }
}
