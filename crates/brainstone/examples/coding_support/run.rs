use brainstone::runtime::Operation;
use brainstone::{Address, Outcome, Prompt};

pub(crate) fn prompt(action: &super::input::Action) -> Result<Prompt, super::Error> {
    let (request, task) = match action {
        super::input::Action::Run { request_id, task } => (request_id.as_str(), task.as_str()),
        _ => ("coding-demo-v1", super::profile::DEMO_TASK),
    };
    Ok(Prompt::new(
        Address::new("coding", "workspace", "main")?,
        request,
        super::profile::SYSTEM,
        task,
    )?
    .with_profile(super::profile::execution()?))
}
pub(crate) fn show(operation: Operation) -> Result<(), super::Error> {
    match operation.outcome {
        Some(Outcome::Completed { text }) => {
            println!("{text}");
            Ok(())
        }
        Some(Outcome::Failed { code, message }) => Err(format!("{code}: {message}").into()),
        _ => Err("No supported terminal outcome".into()),
    }
}
