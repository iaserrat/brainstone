use opentelemetry::{KeyValue, Value};
pub(crate) const IDS: [&str; 6] = [
    "operation_id",
    "session_id",
    "lane_id",
    "parent_operation_id",
    "child_operation_id",
    "attempt",
];
pub(crate) fn attribute(attr: &KeyValue) -> bool {
    match (attr.key.as_str(), &attr.value) {
        ("kind", Value::String(value)) => crate::schema::KINDS.contains(&value.as_str()),
        ("outcome", Value::String(value)) => crate::schema::OUTCOMES.contains(&value.as_str()),
        (name, Value::I64(value)) => IDS.contains(&name) && *value >= 0,
        _ => false,
    }
}
