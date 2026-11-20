//! Provider-neutral checks for the object input shape native tool APIs accept.
use crate::{OutputSchema, ToolError};
use schemars::{JsonSchema, generate::SchemaSettings};

const CODE_INVALID_DEFINITION: &str = "invalid_tool_definition";

pub(super) fn schema<A: JsonSchema>(
    name: &str,
    description: &str,
) -> Result<OutputSchema, ToolError> {
    if description.trim().is_empty() {
        return Err(ToolError::new(
            CODE_INVALID_DEFINITION,
            "Tool description must not be blank",
        ));
    }
    let generated = SchemaSettings::draft2020_12()
        .for_deserialize()
        .into_generator()
        .into_root_schema_for::<A>()
        .to_value();
    if generated["type"] != "object" {
        return Err(ToolError::new(
            CODE_INVALID_DEFINITION,
            "Tool arguments require an object-root schema; wrap input in a Rust struct",
        ));
    }
    OutputSchema::new(name, generated)
        .map_err(|error| ToolError::new(CODE_INVALID_DEFINITION, error.to_string()))
}
