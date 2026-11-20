//! Typed closures use the same tool seam as hand-written and remote tools.

use crate::{OutputSchema, ToolError, ToolInvocation};
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt, marker::PhantomData};

#[path = "typed_tool_call.rs"]
mod call;
#[path = "typed_tool_definition.rs"]
mod definition;

/// A typed async handler with its compiled input schema.
///
/// Register it as an ordinary [`crate::Tool`]. Invalid arguments never reach
/// the handler. Successful output is one JSON document with `outcome: "success"`
/// and `value`; domain errors retain the code returned by `Into<ToolError>`.
/// The driver encodes failures as `{"error": code, "message": detail}`.
pub struct TypedTool<A, F> {
    name: String,
    description: String,
    schema: OutputSchema,
    handler: F,
    arguments: PhantomData<fn(A)>,
}

/// Derive a tool's input schema and adapt an async typed function.
///
/// Input must derive an object-root schema, normally a Rust struct; wrap scalar
/// inputs and root enums in a struct. Nested enums and recursive fields work.
/// Uses Serde's deserialization shape, including defaults and renamed fields.
/// Names contain 1–64 ASCII letters, digits, underscores, or hyphens; descriptions
/// must contain at least one non-whitespace character.
/// The handler receives decoded arguments and driver-authored invocation metadata.
/// Output must serialize as JSON; attachments still use [`crate::Tool`] directly.
/// External effects must honor the invocation's retry key themselves.
///
/// # Errors
/// Returns `invalid_tool_definition` for invalid names, blank descriptions,
/// unsupported root argument shapes, or invalid schemas. Calls return
/// `invalid_tool_arguments` before invoking the handler when validation fails,
/// and `tool_output_invalid` if output cannot serialize or exceeds
/// [`crate::MAX_TOOL_RESULT_BYTES`]. Oversized JSON is rejected, never truncated.
pub fn typed_tool<A, O, E, F, Fut>(
    name: &str,
    description: &str,
    handler: F,
) -> Result<TypedTool<A, F>, ToolError>
where
    A: DeserializeOwned + JsonSchema + Send + 'static,
    O: Serialize,
    E: Into<ToolError>,
    F: Fn(A, ToolInvocation) -> Fut + Send + Sync,
    Fut: Future<Output = Result<O, E>> + Send,
{
    let schema = definition::schema::<A>(name, description)?;
    Ok(TypedTool {
        name: name.to_owned(),
        description: description.to_owned(),
        schema,
        handler,
        arguments: PhantomData,
    })
}

impl<A, F> fmt::Debug for TypedTool<A, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypedTool")
            .field("name", &self.name)
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}
