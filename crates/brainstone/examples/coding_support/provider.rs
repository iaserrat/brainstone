use brainstone::openai::{OPENAI_RESPONSES_URL, ResponsesModel};

pub(crate) fn live() -> Result<ResponsesModel, super::Error> {
    let endpoint = std::env::var("OPENAI_ENDPOINT").unwrap_or_else(|_| OPENAI_RESPONSES_URL.into());
    Ok(ResponsesModel::new(
        &endpoint,
        std::env::var("OPENAI_MODEL")?,
        std::env::var("OPENAI_API_KEY")?,
    )?)
}
