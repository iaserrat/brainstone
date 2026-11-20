use crate::Error;

/// Every event schema this binary interprets; anything newer fails closed.
pub(crate) fn check_version(version: i16) -> Result<(), Error> {
    if (1..=ATTACHMENT_SCHEMA_VERSION).contains(&version) {
        return Ok(());
    }
    Err(Error::CorruptRecord {
        message: format!("unsupported event schema version: {version}"),
    })
}

pub(crate) fn missing_payload() -> Error {
    Error::CorruptRecord {
        message: "stored event has no payload".to_owned(),
    }
}

pub(crate) const ATTACHMENT_SCHEMA_VERSION: i16 = 2;

pub(crate) fn version(event: &agent::Event) -> i16 {
    let results = match event {
        agent::Event::Appended {
            message: agent::Message::ToolResults(results),
        } => results.as_slice(),
        _ => &[],
    };
    if results.iter().any(|result| !result.images.is_empty()) {
        return ATTACHMENT_SCHEMA_VERSION;
    }
    1
}
