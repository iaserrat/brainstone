//! Read-only fixture lookup. Replace the catalog with application data.
use brainstone::{Tool, ToolError, typed_tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LookupOrder {
    #[schemars(length(min = 1))]
    order_id: String,
}

#[derive(Serialize)]
struct Order {
    order_id: String,
    status: &'static str,
}

enum LookupError {
    InvalidArguments,
    NotFound,
}

pub(crate) fn tool() -> Result<impl Tool, ToolError> {
    typed_tool(
        "LookupOrder",
        "Look up a demo order by its exact ID, such as BS-1001.",
        |args: LookupOrder, _| async move { lookup(args) },
    )
}

fn lookup(args: LookupOrder) -> Result<Order, LookupError> {
    if args.order_id.trim().is_empty() {
        return Err(LookupError::InvalidArguments);
    }
    let status = match args.order_id.as_str() {
        "BS-1001" => "shipped",
        "BS-1002" => "processing",
        _ => return Err(LookupError::NotFound),
    };
    Ok(Order {
        order_id: args.order_id,
        status,
    })
}

impl From<LookupError> for ToolError {
    fn from(error: LookupError) -> Self {
        match error {
            LookupError::InvalidArguments => {
                Self::new("invalid_arguments", "Supply a non-blank order_id.")
            }
            LookupError::NotFound => Self::new("order_not_found", "No demo order has that ID."),
        }
    }
}
