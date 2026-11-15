//! The database definition of prompt idempotency: original surface text,
//! attachments, and delivery target must match. Model-visible text may differ
//! because admission-time skill or memory enrichment changed. Both operation
//! and steering duplicates check here so the definition cannot drift.

use serde_json::Value;
use sqlx::types::Json;

use crate::{Error, Prompt, PromptContent};

pub(crate) fn check(
    prompt: &Prompt,
    stored: &PromptContent,
    source: &str,
    target: Option<&Json<Value>>,
) -> Result<(), Error> {
    let content_matches =
        source == prompt.source_text && stored.images == prompt.content.images;
    if !content_matches || target != delivery_json(prompt)?.as_ref() {
        return Err(Error::IdempotencyConflict);
    }
    Ok(())
}

fn delivery_json(prompt: &Prompt) -> Result<Option<Json<Value>>, Error> {
    prompt
        .delivery
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(Error::corrupt)
        .map(|value| value.map(Json))
}
