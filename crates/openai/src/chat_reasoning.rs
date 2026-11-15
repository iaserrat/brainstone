use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn visible(delta: &Value, known: &BTreeMap<u64, Value>) -> String {
    let text: String = delta["reasoning_details"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| detail(item, known))
        .collect();
    if !text.is_empty() {
        return text;
    }
    delta["reasoning"]
        .as_str()
        .filter(|text| !text.is_empty())
        .or_else(|| delta["reasoning_content"].as_str())
        .unwrap_or_default()
        .to_owned()
}
fn detail<'a>(item: &'a Value, known: &BTreeMap<u64, Value>) -> Option<&'a str> {
    let index = item["index"].as_u64()?;
    let kind = item["type"]
        .as_str()
        .or_else(|| known.get(&index)?["type"].as_str())?;
    let key = match kind {
        "reasoning.text" => "text",
        "reasoning.summary" => "summary",
        _ => return None,
    };
    item[key].as_str()
}
