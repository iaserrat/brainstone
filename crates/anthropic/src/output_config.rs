//! Output shape and effort share one Messages field but remain independent.
use agent::OutputSchema;
use serde::Serialize;
use serde_json::Value;

use crate::Effort;

#[derive(Serialize)]
pub(crate) struct OutputConfig<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<Effort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<Format<'a>>,
}

#[derive(Serialize)]
struct Format<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    schema: &'a Value,
}

impl<'a> OutputConfig<'a> {
    pub(crate) fn new(effort: Option<Effort>, schema: Option<&'a OutputSchema>) -> Option<Self> {
        if effort.is_none() && schema.is_none() {
            return None;
        }
        Some(Self {
            effort,
            format: schema.map(|schema| Format {
                kind: "json_schema",
                schema: schema.schema(),
            }),
        })
    }
}
