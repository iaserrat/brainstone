//! The same application contracts run against both durable backends.
use brainstone_runtime::{Address, Prompt};
#[path = "support/durable.rs"]
mod contracts;
#[path = "support/database.rs"]
mod database;
#[path = "support/backend_restart.rs"]
mod restart;
#[path = "support/subagent_retries.rs"]
mod retries;
#[path = "support/standing.rs"]
mod standing;

#[tokio::test]
async fn sqlite_implements_the_complete_application_contract() {
    let database = database::Database::sqlite();
    check(&database.url).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_implements_the_same_application_contract() {
    let database = database::Database::postgres().await;
    check(&database.url).await;
    database.close().await;
}

async fn check(url: &str) {
    let store = brainstone_runtime::DurableStore::connect(url)
        .await
        .unwrap();
    contracts::ancillary(&store, "backend-contract").await;
    contracts::schedules(&store, "backend-contract").await;
    contracts::posts(&store, "backend-contract").await;
    let prompt = Prompt::new(
        Address::new("test", "backend-contract", "child").unwrap(),
        "parent",
        "s",
        "start",
    )
    .unwrap();
    retries::check(&store, &prompt).await;
    standing::check(url, &store).await;
    store.close().await;
    restart::check(url).await;
}
