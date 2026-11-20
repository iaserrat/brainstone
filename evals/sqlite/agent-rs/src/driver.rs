//! Native host acceptance, discovery and durable state polling.
use crate::{
    model::agent,
    protocol::{Action, Error, Input, crash, emit},
    state,
};
use agent_rs::{Agent, DurableStore, Host, HostOptions, RunId, sqlite::SqliteStore};
use serde_json::json;
use std::{sync::Arc, time::Duration};

pub async fn run(input: Input) -> Result<(), Error> {
    let store = Arc::new(SqliteStore::open(&input.database).await?);
    if input.action == Action::Inspect {
        state::emit_state(&store.status(operation(&input)?).await?);
        store.close().await?;
        return Ok(());
    }
    let agent = agent(&input)?;
    let id = accept(&input, &store, &agent).await?;
    if let Some(id) = id {
        drive(store.clone(), agent, id).await?;
    }
    store.close().await?;
    Ok(())
}

async fn accept(
    input: &Input,
    store: &SqliteStore,
    agent: &Agent<String, String>,
) -> Result<Option<RunId>, Error> {
    if input.action == Action::Resume {
        return Ok(Some(operation(input)?));
    }
    let result = store
        .accept(
            &agent.erased(),
            &input.conversation,
            &input.request_id,
            json!(input.text),
            None,
        )
        .await;
    match result {
        Ok(id) => {
            emit(&json!({"event":"accepted", "operation_id":id.to_string()}));
            crash(input.fault.as_deref(), "after_accept");
            Ok(Some(id))
        }
        Err(error) => {
            crate::protocol::error(&error);
            Ok(None)
        }
    }
}

fn operation(input: &Input) -> Result<RunId, Error> {
    Ok(input
        .operation_id
        .as_deref()
        .ok_or("operation_id_required")?
        .parse()?)
}

async fn drive(
    store: Arc<SqliteStore>,
    agent: Agent<String, String>,
    id: RunId,
) -> Result<(), Error> {
    let options = HostOptions::default().with_max_concurrent_runs(1);
    let mut host = Host::start(store, agent, options)?;
    let result = tokio::time::timeout(Duration::from_secs(90), poll(&host, id)).await;
    host.shutdown().await?;
    result??;
    Ok(())
}

async fn poll(host: &Host<SqliteStore>, id: RunId) -> agent_rs::Result<()> {
    loop {
        let state = host.status(id).await?;
        if state.is_terminal() || state::waiting(&state) {
            state::emit_state(&state);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
