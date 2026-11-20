use crate::failure::{CODE_INVALID_REQUEST, Failure};
use serde::Serialize;
use serde_json::{Map, Value};

const RESERVED: &[&str] = &[
    "model",
    "messages",
    "input",
    "instructions",
    "tools",
    "stream",
    "store",
    "previous_response_id",
    "conversation",
    "background",
    "n",
];

/// Provider-specific request settings. Conversation, tools, terminal handling,
/// and remote persistence remain owned by the adapter.
#[derive(Clone, Default, Debug)]
pub struct RequestParameters(Map<String, Value>);

impl RequestParameters {
    /// # Errors
    /// Rejects fields that could override the harness's conversation contract.
    pub fn new(values: Map<String, Value>) -> Result<Self, ParameterError> {
        if let Some(name) = values.keys().find(|name| RESERVED.contains(&name.as_str())) {
            return Err(ParameterError { name: name.clone() });
        }
        Ok(Self(values))
    }

    pub(crate) fn encode<T: Serialize + ?Sized>(&self, body: &T) -> Result<Vec<u8>, Failure> {
        if self.0.is_empty() {
            return serde_json::to_vec(body).map_err(serialization);
        }
        let mut body = serde_json::to_value(body).map_err(serialization)?;
        let object = body.as_object_mut().expect("provider requests are objects");
        for (key, value) in &self.0 {
            if object.contains_key(key) {
                return Err(Failure::new(
                    CODE_INVALID_REQUEST,
                    format!("duplicate request parameter: {key}"),
                ));
            }
            object.insert(key.clone(), value.clone());
        }
        serde_json::to_vec(&body).map_err(serialization)
    }
}

/// Invalid operator-owned request settings. Values are excluded from diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterError {
    name: String,
}

impl ParameterError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(&self) -> &'static str {
        "invalid_request_parameter"
    }
}
impl std::fmt::Display for ParameterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "reserved request parameter: {}", self.name)
    }
}
impl std::error::Error for ParameterError {}

#[allow(clippy::needless_pass_by_value, reason = "map_err callback")]
fn serialization(error: serde_json::Error) -> Failure {
    Failure::new(CODE_INVALID_REQUEST, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::RequestParameters;
    use serde_json::{Map, json};

    #[test]
    fn unstructured_requests_keep_legacy_output_parameter_passthrough() {
        for name in ["response_format", "text"] {
            let value = json!({"type":"json_object"});
            let parameters = Map::from_iter([(name.to_owned(), value.clone())]);
            let parameters = RequestParameters::new(parameters).unwrap();
            let bytes = parameters.encode(&json!({"model":"fixture"})).ok().unwrap();
            let encoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(encoded[name], value);
        }
    }
}
