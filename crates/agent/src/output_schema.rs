use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::OutputError;

#[path = "output_reference.rs"]
mod reference;

const MAX_NAME_BYTES: usize = 64;

/// An immutable JSON Schema contract for a model's final JSON document.
///
/// Construction validates and compiles the schema. Clones share the compiled
/// validator. Built-in formats are checked; only bundled local references resolve.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawOutputSchema")]
pub struct OutputSchema {
    name: String,
    schema: Value,
    #[serde(skip)]
    compiled: Arc<jsonschema::Validator>,
}

impl OutputSchema {
    /// Build a named contract without reading files or fetching remote schemas.
    ///
    /// # Errors
    /// Returns `invalid_output_schema` for invalid names, invalid schemas, or
    /// references that require an external resource. Local `$defs` references work.
    pub fn new(name: &str, schema: Value) -> Result<Self, OutputError> {
        if name.is_empty() || name.len() > MAX_NAME_BYTES || !name.bytes().all(name_character) {
            return Err(OutputError::InvalidName);
        }
        let compiled = jsonschema::options()
            .with_retriever(reference::Offline)
            .should_validate_formats(true)
            .build(&schema)
            .map_err(|_| OutputError::InvalidSchema)?;
        Ok(Self {
            name: name.to_owned(),
            schema,
            compiled: Arc::new(compiled),
        })
    }

    #[must_use]
    /// The provider-visible name of this output contract.
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// The original JSON Schema, preserved for provider requests and durable records.
    pub const fn schema(&self) -> &Value {
        &self.schema
    }

    /// Parse and validate one complete JSON document against this contract.
    ///
    /// # Errors
    /// Returns `structured_output_invalid` for invalid JSON or a schema mismatch.
    pub fn validate(&self, text: &str) -> Result<Value, OutputError> {
        let value = serde_json::from_str(text).map_err(|_| OutputError::InvalidJson)?;
        self.compiled
            .validate(&value)
            .map_err(|_| OutputError::SchemaMismatch)?;
        Ok(value)
    }
}

impl PartialEq for OutputSchema {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.schema == other.schema
    }
}
impl Eq for OutputSchema {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutputSchema {
    name: String,
    schema: Value,
}

impl TryFrom<RawOutputSchema> for OutputSchema {
    type Error = OutputError;

    fn try_from(raw: RawOutputSchema) -> Result<Self, Self::Error> {
        Self::new(&raw.name, raw.schema)
    }
}

fn name_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}
