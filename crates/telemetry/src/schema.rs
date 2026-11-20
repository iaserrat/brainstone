use tracing::field::{Field, Visit};

pub(crate) const KINDS: [&str; 10] = [
    "operation",
    "model",
    "tool",
    "mcp.queue",
    "mcp.call",
    "subagent",
    "delivery",
    "provider",
    "compaction",
    "recovery",
];
pub(crate) const OUTCOMES: [&str; 6] = [
    "ok",
    "failed",
    "cancelled",
    "retry",
    "lease_lost",
    "abandoned",
];

pub(crate) struct Fields {
    pub(crate) kind: &'static str,
    pub(crate) outcome: &'static str,
    pub(crate) ids: std::collections::BTreeMap<&'static str, u64>,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            kind: "unknown",
            outcome: "abandoned",
            ids: std::collections::BTreeMap::new(),
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "kind" => {
                self.kind = KINDS
                    .iter()
                    .copied()
                    .find(|item| *item == value)
                    .unwrap_or("unknown");
            }
            "outcome" => {
                self.outcome = OUTCOMES
                    .iter()
                    .copied()
                    .find(|item| *item == value)
                    .unwrap_or("abandoned");
            }
            _ => {}
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "dropped_span_count" {
            self.ids.insert("dropped_span_count", value);
        }

        if let Some(name) = crate::safe_fields::IDS
            .iter()
            .find(|name| **name == field.name())
        {
            self.ids.insert(name, value);
        }
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.record_u64(field, value);
        }
    }
    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}

pub(crate) fn enabled(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.is_span() && metadata.target() == "brainstone::telemetry" && metadata.name() == "work"
}
