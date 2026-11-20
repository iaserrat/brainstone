use agent::Registry;

use crate::{Catalog, Definition, Error};

pub(crate) fn validate(
    definition: &Definition,
    catalog: &Catalog,
    available: &Registry,
    selected: &Registry,
) -> Result<(), Error> {
    validate_mcp(definition, available)?;
    validate_delegation(definition, catalog)?;
    validate_tools(definition, available)?;
    if selected.definitions().is_empty() {
        return Err(Error::NoTools {
            name: definition.name().to_owned(),
        });
    }
    Ok(())
}

fn validate_tools(definition: &Definition, available: &Registry) -> Result<(), Error> {
    for tool in definition.tools().into_iter().flatten() {
        let pattern = vec![tool.clone()];
        if available
            .select(Some(&pattern), &[])
            .definitions()
            .is_empty()
        {
            return Err(Error::UnknownTool {
                agent: definition.name().to_owned(),
                tool: tool.clone(),
            });
        }
    }
    Ok(())
}

fn validate_mcp(definition: &Definition, available: &Registry) -> Result<(), Error> {
    let names: Vec<String> = available
        .definitions()
        .iter()
        .filter_map(|value| value["name"].as_str())
        .map(str::to_owned)
        .collect();
    for server in definition.mcp_servers() {
        let prefix = format!("mcp__{server}__");
        if !names.iter().any(|name| name.starts_with(&prefix)) {
            return Err(Error::UnknownMcpServer {
                name: server.clone(),
            });
        }
    }
    Ok(())
}

fn validate_delegation(definition: &Definition, catalog: &Catalog) -> Result<(), Error> {
    for tool in definition.tools().into_iter().flatten() {
        let Some(values) = tool
            .strip_prefix("Agent(")
            .and_then(|value| value.strip_suffix(')'))
        else {
            continue;
        };
        for name in values.split(',').map(str::trim) {
            if catalog.get(name).is_none() {
                return Err(Error::UnknownAgent {
                    name: name.to_owned(),
                });
            }
        }
    }
    Ok(())
}
