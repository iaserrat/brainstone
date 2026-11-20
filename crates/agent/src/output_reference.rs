//! Schema compilation never reads files or fetches network resources.
use jsonschema::{Retrieve, Uri};
use serde_json::Value;

pub(super) struct Offline;

impl Retrieve for Offline {
    fn retrieve(&self, _: &Uri<String>) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("external output schema references are disabled".into())
    }
}
