use crate::Error;

pub(crate) struct Config {
    pub(crate) traces: bool,
    pub(crate) metrics: bool,
}
impl Config {
    pub(crate) fn from_env() -> Result<Self, Error> {
        Self::read(|key| std::env::var(key).ok())
    }
    pub(crate) fn read(get: impl Fn(&str) -> Option<String>) -> Result<Self, Error> {
        if get("OTEL_SDK_DISABLED").as_deref() == Some("true") {
            return Ok(Self {
                traces: false,
                metrics: false,
            });
        }
        let configured = [
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
        ]
        .iter()
        .any(|key| get(key).is_some());
        validate(&get)?;
        Ok(Self {
            traces: signal(get("OTEL_TRACES_EXPORTER").as_deref(), configured)?,
            metrics: signal(get("OTEL_METRICS_EXPORTER").as_deref(), configured)?,
        })
    }
}
fn signal(value: Option<&str>, configured: bool) -> Result<bool, Error> {
    match value {
        None => Ok(configured),
        Some("otlp") => Ok(true),
        Some("none") => Ok(false),
        _ => Err(Error::Configuration),
    }
}

fn validate(get: &impl Fn(&str) -> Option<String>) -> Result<(), Error> {
    for key in [
        "OTEL_EXPORTER_OTLP_PROTOCOL",
        "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL",
        "OTEL_EXPORTER_OTLP_METRICS_PROTOCOL",
    ] {
        if get(key).is_some_and(|value| value != "http/protobuf") {
            return Err(Error::Configuration);
        }
    }
    let sampler = get("OTEL_TRACES_SAMPLER");
    match sampler.as_deref() {
        None
        | Some("always_on" | "always_off" | "parentbased_always_on" | "parentbased_always_off") => {
            Ok(())
        }
        Some("traceidratio" | "parentbased_traceidratio") => {
            let ratio = get("OTEL_TRACES_SAMPLER_ARG").and_then(|value| value.parse::<f64>().ok());
            if ratio.is_some_and(|value| (0.0..=1.0).contains(&value)) {
                Ok(())
            } else {
                Err(Error::Configuration)
            }
        }
        _ => Err(Error::Configuration),
    }
}
