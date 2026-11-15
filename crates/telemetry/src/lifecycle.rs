use crate::schema::Fields;
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram, MeterProvider as _},
};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::time::Instant;
use tracing::{
    Subscriber,
    span::{Attributes, Id, Record},
};
use tracing_subscriber::{layer::Context, registry::LookupSpan};

// Seconds: distinguish short tool waits and model/operation work up to ten minutes.
const DURATION_BUCKETS: [f64; 14] = [
    0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0,
];

pub(crate) struct Layer {
    started: Counter<u64>,
    finished: Counter<u64>,
    duration: Histogram<f64>,
}
pub(crate) struct State {
    started: Instant,
    pub(crate) fields: Fields,
}

impl Layer {
    pub(crate) fn new(provider: &SdkMeterProvider) -> Self {
        let meter = provider.meter("brainstone");
        Self {
            started: meter.u64_counter("brainstone.work.started").build(),
            finished: meter.u64_counter("brainstone.work.finished").build(),
            duration: meter
                .f64_histogram("brainstone.work.duration")
                .with_unit("s")
                .with_boundaries(DURATION_BUCKETS.to_vec())
                .build(),
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> tracing_subscriber::Layer<S> for Layer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if !crate::schema::enabled(attrs.metadata()) {
            return;
        }
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        self.started.add(1, &[KeyValue::new("kind", fields.kind)]);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(State {
                started: Instant::now(),
                fields,
            });
        }
    }
    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(state) = span.extensions_mut().get_mut::<State>()
        {
            values.record(&mut state.fields);
        }
    }
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let extensions = span.extensions();
        let Some(state) = extensions.get::<State>() else {
            return;
        };
        let attrs = [
            KeyValue::new("kind", state.fields.kind),
            KeyValue::new("outcome", state.fields.outcome),
        ];
        self.finished.add(1, &attrs);
        self.duration
            .record(state.started.elapsed().as_secs_f64(), &attrs);
    }
}
