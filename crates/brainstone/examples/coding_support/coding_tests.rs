//! Subprocesses prove the offline example resumes the same edited workspace.
#![allow(clippy::too_many_lines)] // End-to-end scenario assertions.
use brainstone::{
    agent::{Event, Message},
    runtime::{DurableStore, OperationId},
};
use std::{
    path::Path,
    process::{Command, Output},
};

#[tokio::test]
async fn demo_edits_checks_and_replays_one_terminal_answer() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("project");
    let first = child(&workspace, false);
    assert!(first.status.success(), "{first:?}");
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        super::demo::after()
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains("\"checks\":\"passed\""));
    let again = child(&workspace, false);
    assert!(again.status.success(), "{again:?}");
    assert_eq!(answer(&first), answer(&again));
    assert!(!String::from_utf8_lossy(&again.stderr).contains("Approval:"));
}

#[tokio::test]
async fn crash_after_file_commit_resumes_without_resetting_the_fixture() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("project");
    let crash = child(&workspace, true);
    assert_eq!(crash.status.code(), Some(73), "{crash:?}");
    assert_eq!(
        std::fs::read_to_string(workspace.join("src/lib.rs")).unwrap(),
        super::demo::after()
    );
    let resumed = child(&workspace, false);
    assert!(resumed.status.success(), "{resumed:?}");
    let again = child(&workspace, false);
    assert!(again.status.success(), "{again:?}");
    assert_eq!(answer(&resumed), answer(&again));
    assert_eq!(committed_edits(&workspace, operation_id(&crash)).await, 1);
}

#[test]
fn demo_never_overwrites_an_existing_project() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("notes.txt"), "keep me").unwrap();
    assert!(super::demo::prepare(directory.path()).is_err());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("notes.txt")).unwrap(),
        "keep me"
    );
}

fn child(workspace: &Path, crash: bool) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "tests::worker", "--nocapture"])
        .env("BRAINSTONE_CODING_TEST_ROOT", workspace)
        .env(
            "BRAINSTONE_CODING_TEST_CRASH",
            if crash { "yes" } else { "no" },
        )
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_MODEL")
        .output()
        .unwrap()
}

#[tokio::test]
#[ignore = "subprocess fixture"]
async fn worker() {
    if std::env::var_os("BRAINSTONE_CODING_TEST_EOF").is_some() {
        assert!(!super::console::Console::new().decide().await.unwrap());
        return;
    }
    let root = std::env::var_os("BRAINSTONE_CODING_TEST_ROOT")
        .unwrap()
        .into();
    let crash = std::env::var("BRAINSTONE_CODING_TEST_CRASH").unwrap() == "yes";
    super::execute(super::input::Options {
        root,
        action: super::input::Action::Demo { crash },
    })
    .await
    .unwrap();
}

fn answer(output: &Output) -> serde_json::Value {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let start = line.find('{')?;
            serde_json::from_str(&line[start..]).ok()
        })
        .expect("a JSON final answer")
}

#[test]
fn copied_state_cannot_redirect_effects_into_another_workspace() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    super::state::database(first.path()).unwrap();
    std::fs::create_dir(second.path().join(".brainstone")).unwrap();
    std::fs::copy(
        first.path().join(".brainstone/workspace-root"),
        second.path().join(".brainstone/workspace-root"),
    )
    .unwrap();
    assert!(super::state::database(second.path()).is_err());
}

#[tokio::test]
async fn state_keeps_unicode_and_url_characters_in_the_workspace_path() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("Zoë %25?#");
    std::fs::create_dir(&root).unwrap();
    let url = super::state::database(&root).unwrap();
    let store = DurableStore::connect(&url).await.unwrap();
    store.close().await;
    assert!(root.join(".brainstone/state.db").is_file());
}

#[test]
fn an_unbound_database_is_not_adopted_as_new_workspace_state() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join(".brainstone")).unwrap();
    std::fs::write(directory.path().join(".brainstone/state.db"), []).unwrap();
    assert!(super::state::database(directory.path()).is_err());
    assert!(!directory.path().join(".brainstone/workspace-root").exists());
}

fn operation_id(output: &Output) -> OperationId {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let number = stderr
        .lines()
        .find_map(|line| line.strip_prefix("operation="))
        .unwrap();
    OperationId::new(number.split_whitespace().next().unwrap().parse().unwrap()).unwrap()
}

async fn committed_edits(workspace: &Path, operation: OperationId) -> usize {
    let root = workspace.canonicalize().unwrap();
    let store = DurableStore::connect(&super::state::database(&root).unwrap())
        .await
        .unwrap();
    let events = store.journal(operation, -1, 100).await.unwrap();
    let count = events.iter().filter(|(_, event)| matches!(event, Event::Appended { message: Message::ToolResults(results) } if results.iter().any(|result| result.name == "EditFile" && !result.is_error))).count();
    store.close().await;
    count
}

#[test]
fn closed_operator_input_denies_and_does_not_hold_process_shutdown_open() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "tests::worker", "--nocapture"])
        .env("BRAINSTONE_CODING_TEST_EOF", "yes")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
