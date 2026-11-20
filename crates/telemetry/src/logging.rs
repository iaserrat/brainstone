use crate::schema::Fields;
use opentelemetry::trace::TraceContextExt as _;
use serde_json::{Value, json};
use std::fmt;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

pub(crate) struct SafeFormat {
    identity: serde_json::Map<String, Value>,
}
impl SafeFormat {
    pub(crate) fn new(resource: &opentelemetry_sdk::Resource) -> Self {
        let mut identity = serde_json::Map::new();
        for (key, attribute) in [
            ("service", "service.name"),
            ("env", "deployment.environment.name"),
            ("version", "service.version"),
        ] {
            if let Some(value) = resource.get(&opentelemetry::Key::new(attribute)) {
                identity.insert(key.into(), value.to_string().into());
            }
        }
        Self { identity }
    }
}
impl<S, N> FormatEvent<S, N> for SafeFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        let mut log = json!({"level": metadata.level().as_str(), "target": metadata.target(), "event": metadata.name()});
        if fields.outcome != "abandoned" {
            log["outcome"] = fields.outcome.into();
        }
        for (key, value) in &fields.ids {
            log[*key] = (*value).into();
        }
        log.as_object_mut()
            .expect("log is an object")
            .extend(self.identity.clone());
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                if let Some(state) = span.extensions().get::<crate::lifecycle::State>() {
                    log["kind"] = state.fields.kind.into();
                    for (key, value) in &state.fields.ids {
                        log[*key] = (*value).into();
                    }
                }
            }
        }
        correlate(&mut log);
        writeln!(writer, "{log}")
    }
}
fn correlate(log: &mut Value) {
    let context = crate::current_context();
    let span = context.span();
    let sc = span.span_context();
    if sc.is_valid() {
        log["trace_id"] = sc.trace_id().to_string().into();
        log["span_id"] = sc.span_id().to_string().into();
        // Datadog's JSON log correlation uses the low 64 bits as decimal strings.
        let bytes = sc.trace_id().to_bytes();
        let low = u64::from_be_bytes(bytes[8..].try_into().expect("trace ID is 16 bytes"));
        log["dd.trace_id"] = low.to_string().into();
        log["dd.span_id"] = u64::from_be_bytes(sc.span_id().to_bytes())
            .to_string()
            .into();
    }
}
