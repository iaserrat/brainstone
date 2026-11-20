use std::collections::BTreeMap;

use crate::error::Error;

const CHILD_ENV: [&str; 5] = ["HOME", "LANG", "PATH", "TMPDIR", "USER"];

pub(crate) fn child(
    refs: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Error> {
    let mut values: BTreeMap<String, String> = CHILD_ENV
        .into_iter()
        .filter_map(|name| {
            environment
                .get(name)
                .map(|value| (name.into(), value.clone()))
        })
        .collect();
    values.extend(resolve_map(refs, environment)?);
    Ok(values)
}

pub(crate) fn resolve_map(
    refs: &BTreeMap<String, String>,
    environment: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Error> {
    refs.iter()
        .map(|(target, source)| Ok((target.clone(), required(source, environment)?)))
        .collect()
}

pub(crate) fn resolve_optional(
    reference: Option<&str>,
    environment: &BTreeMap<String, String>,
) -> Result<Option<String>, Error> {
    reference
        .map(|name| required(name, environment))
        .transpose()
}

fn required(name: &str, environment: &BTreeMap<String, String>) -> Result<String, Error> {
    environment
        .get(name)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| Error::new("mcp_configuration", format!("missing environment {name}")))
}
