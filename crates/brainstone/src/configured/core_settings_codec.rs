use crate::configured::ConfigError;
use std::collections::{BTreeMap, HashMap};

pub(super) fn detailed<E: std::fmt::Display>(name: &'static str) -> impl Fn(E) -> ConfigError {
    move |error| ConfigError::Detailed {
        name,
        message: error.to_string(),
    }
}

pub(super) fn ordered(values: &HashMap<String, String>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}
