//! Shared CLI and resource ownership for the two provider-backed examples.
use brainstone::openai::{OPENAI_RESPONSES_URL, ResponsesModel};
use brainstone::runtime::SqliteStore;
use brainstone::{Address, Host, Outcome, Prompt, Registry, Runtime};

type Error = Box<dyn std::error::Error>;

pub(crate) async fn run(agent: &str, system: &str, tools: Registry) -> Result<(), Error> {
    let (request_id, text) = input()?;
    let endpoint =
        std::env::var("OPENAI_ENDPOINT").unwrap_or_else(|_| OPENAI_RESPONSES_URL.to_owned());
    let model = ResponsesModel::new(
        &endpoint,
        std::env::var("OPENAI_MODEL")?,
        std::env::var("OPENAI_API_KEY")?,
    )?;
    let url = database_url(agent)?;
    let conversation = std::env::var("CONVERSATION_ID").unwrap_or_else(|_| "local".to_owned());
    let address = Address::new(agent, &conversation, "main")?;
    let prompt = Prompt::new(address, &request_id, system, &text)?;
    let store = SqliteStore::connect(&url).await?;
    let host = Host::new(Runtime::new(store.clone()), model, tools);
    let outcome = execute(&host, &prompt).await;
    host.shutdown().await;
    store.close().await;
    show(outcome?)
}

fn input() -> Result<(String, String), Error> {
    let mut args = std::env::args().skip(1);
    let id = args.next().ok_or("usage: <request-id> <quoted-message>")?;
    let text = args.next().ok_or("usage: <request-id> <quoted-message>")?;
    if args.next().is_some() {
        return Err("Quote the message as one argument.".into());
    }
    Ok((id, text))
}

fn database_url(agent: &str) -> Result<String, Error> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        return Ok(url);
    }
    std::fs::create_dir_all("data")?;
    Ok(format!("sqlite://data/{agent}.db"))
}

async fn execute(
    host: &Host<SqliteStore, ResponsesModel>,
    prompt: &Prompt,
) -> Result<Option<Outcome>, host::Error> {
    let accepted = host.accept(prompt).await?;
    eprintln!(
        "operation={} duplicate={}",
        accepted.operation.id.get(),
        accepted.duplicate
    );
    Ok(host.execute(accepted.operation.id).await?.outcome)
}

fn show(outcome: Option<Outcome>) -> Result<(), Error> {
    match outcome {
        Some(Outcome::Completed { text }) => {
            println!("{text}");
            Ok(())
        }
        Some(Outcome::Failed { code, message }) => Err(format!("{code}: {message}").into()),
        _ => Err("No supported terminal outcome.".into()),
    }
}
