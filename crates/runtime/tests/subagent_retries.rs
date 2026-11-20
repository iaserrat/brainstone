use brainstone_runtime::{Address, MemoryStore, Prompt};

#[path = "support/subagent_retries.rs"]
mod contract;

#[tokio::test]
async fn followup_retries_preserve_the_current_child_task() {
    let address = Address::new("test", "subagent-retries", "main").unwrap();
    let prompt = Prompt::new(address, "parent", "system", "task").unwrap();
    contract::check(&MemoryStore::default(), &prompt).await;
}
