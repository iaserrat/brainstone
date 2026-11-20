#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Resident-runtime workload: one process drains accepted work until shutdown.
#![allow(clippy::too_many_lines)] // Workload orchestration and distribution assertions.
use agent::{Model, ModelResponse, Outcome, Registry, StopReason, Thread};
use brainstone_runtime::{Address, DurableStore, Prompt, Runtime};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

struct Answer;
impl Model for Answer {
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        ModelResponse {
            model: None,
            text: "done".into(),
            reasoning: vec![],
            tool_calls: vec![],
            stop: StopReason::EndTurn,
        }
    }
}
#[tokio::test]
#[ignore = "explicit resident load/soak measurement"]
async fn sqlite_resident_soak() {
    let seconds =
        std::env::var("BRAINSTONE_SOAK_SECONDS").map_or(30, |v| v.parse::<u64>().unwrap());
    assert!(seconds > 0);
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("runtime.db");
    let store = DurableStore::connect(&format!("sqlite://{}", database.display()))
        .await
        .unwrap();
    let runtime = Arc::new(Runtime::new(store.clone()));
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let initial_rss = rss();
    let mut workers = Vec::new();
    for session in 0..8 {
        let runtime = runtime.clone();
        workers.push(tokio::spawn(async move {
            worker(&runtime, session, deadline).await
        }));
    }
    let mut samples = Vec::new();
    let mut completed = 0;
    for worker in workers {
        let (count, latencies) = worker.await.unwrap();
        completed += count;
        samples.extend(latencies);
    }
    samples.sort_unstable();
    assert!(completed > 0);
    let mut before = None;
    let mut terminal = 0;
    loop {
        let page = store.operations(before, 100).await.unwrap();
        if page.is_empty() {
            break;
        }
        assert!(page.iter().all(|row| row.state == "completed"));
        terminal += page.len();
        before = Some(brainstone_runtime::OperationId::new(page.last().unwrap().id).unwrap());
    }
    assert_eq!(terminal, completed);
    println!(
        "{}",
        json!({"kind":"resident_sqlite_runtime","workers":8,"completed":completed,
        "seconds":started.elapsed().as_secs_f64(),"sample_window_per_worker":10000,
        "latency_us":{"p50":samples[samples.len()/2],"p95":samples[samples.len()*95/100],"p99":samples[samples.len()*99/100]},
        "rss_kib_before":initial_rss,"rss_kib_after":rss(),"database_bytes":std::fs::metadata(database).unwrap().len()})
    );
    store.close().await;
}
async fn worker(
    runtime: &Runtime<DurableStore>,
    session: usize,
    deadline: Instant,
) -> (usize, Vec<u128>) {
    let mut count = 0;
    let mut samples = Vec::new();
    while Instant::now() < deadline {
        let prompt = Prompt::new(
            Address::new("soak", &session.to_string(), "lane").unwrap(),
            &count.to_string(),
            "system",
            "go",
        )
        .unwrap();
        let start = Instant::now();
        let id = runtime.accept(&prompt).await.unwrap().operation.id;
        let result = runtime
            .run_operation(id, &Answer, &Registry::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.outcome,
            Some(Outcome::Completed {
                text: "done".into()
            })
        );
        let sample = start.elapsed().as_micros();
        if samples.len() < 10000 {
            samples.push(sample);
        } else {
            samples[count % 10000] = sample;
        }
        count += 1;
    }
    (count, samples)
}
fn rss() -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}
