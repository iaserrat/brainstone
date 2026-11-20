use crate::configured::ConfigError;
use agent::ModelCapabilities;
use std::collections::HashMap;

pub(crate) fn parse(
    values: &HashMap<String, String>,
    name: &'static str,
) -> Result<ModelCapabilities, ConfigError> {
    let mut capabilities = ModelCapabilities {
        tools: false,
        images: false,
        tool_images: false,
        reasoning_effort: false,
        streaming: false,
    };
    let Some(value) = values.get(name) else {
        capabilities.tools = true;
        capabilities.streaming = true;
        return Ok(capabilities);
    };
    for capability in value.split(',').map(str::trim) {
        enable(&mut capabilities, capability, name)?;
    }
    Ok(capabilities)
}

fn enable(
    capabilities: &mut ModelCapabilities,
    value: &str,
    name: &'static str,
) -> Result<(), ConfigError> {
    match value {
        "tools" => capabilities.tools = true,
        "images" => capabilities.images = true,
        "tool-images" => capabilities.tool_images = true,
        "reasoning-effort" => capabilities.reasoning_effort = true,
        "streaming" => capabilities.streaming = true,
        "text" => {}
        _ => return Err(ConfigError::Invalid { name }),
    }
    Ok(())
}
