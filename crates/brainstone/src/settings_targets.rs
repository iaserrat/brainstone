use crate::{ConfigError, target_options::TargetOptions};
use std::{borrow::Cow, collections::HashMap};

pub(crate) struct Targets {
    pub(crate) primary: Option<TargetOptions>,
    pub(crate) fallback: Option<TargetOptions>,
}

pub(crate) struct Parsed<'a> {
    pub(crate) values: Cow<'a, HashMap<String, String>>,
    // Target configuration is loaded once; boxing keeps service startup futures bounded.
    pub(crate) targets: Box<Targets>,
}

pub(crate) fn parse(values: &HashMap<String, String>) -> Result<Parsed<'_>, ConfigError> {
    let primary = TargetOptions::parse(values, &crate::target_names::PRIMARY)?;
    let fallback = TargetOptions::parse(values, &crate::target_names::FALLBACK)?;
    let values = crate::settings_aliases::normalize(values)?;
    crate::target_validation::providers(&values, primary.as_ref(), fallback.as_ref())?;
    Ok(Parsed {
        values,
        targets: Box::new(Targets { primary, fallback }),
    })
}
