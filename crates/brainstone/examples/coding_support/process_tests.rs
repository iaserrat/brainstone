//! Child fixtures exercise timeout and streaming bounds without a shell.
use super::*;
use std::io::Write;

fn fixture_command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--ignored",
            "--exact",
            "tools::process::tests::fixture",
            "--nocapture",
        ])
        .env("BRAINSTONE_CODING_PROCESS_FIXTURE", mode);
    command
}

#[tokio::test]
async fn child_exit_status_is_preserved() {
    let output = capture(fixture_command("fail"), Duration::from_secs(5))
        .await
        .unwrap();
    assert!(!output.success);
    assert!(output.stdout.contains("fixture failure"));
}

#[tokio::test]
async fn child_timeout_kills_and_reaps_the_process() {
    let error = capture(fixture_command("wait"), Duration::from_millis(50))
        .await
        .err()
        .unwrap();
    assert_eq!(
        brainstone::ToolError::from(error).code,
        "coding_checks_timeout"
    );
}

#[tokio::test]
async fn child_output_is_bounded_while_it_runs() {
    let error = capture(fixture_command("overflow"), Duration::from_secs(5))
        .await
        .err()
        .unwrap();
    assert_eq!(
        brainstone::ToolError::from(error).code,
        "coding_checks_output_limit"
    );
}

#[test]
#[ignore = "child-process fixture"]
fn fixture() {
    match std::env::var("BRAINSTONE_CODING_PROCESS_FIXTURE")
        .unwrap()
        .as_str()
    {
        "fail" => {
            println!("fixture failure");
            std::process::exit(7);
        }
        "wait" => std::thread::sleep(Duration::from_secs(5)),
        "overflow" => {
            drop(std::io::stdout().write_all(&vec![b'a'; MAX_OUTPUT_BYTES * 2]));
        }
        _ => panic!("unknown fixture"),
    }
}
