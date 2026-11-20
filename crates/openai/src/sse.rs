use crate::failure::{CODE_INVALID_RESPONSE, Failure};

const MAX_EVENT_BYTES: usize = 1_048_576;

#[derive(Default)]
pub(crate) struct Decoder {
    bytes: Vec<u8>,
}

impl Decoder {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() > MAX_EVENT_BYTES {
            return Err(invalid("stream event exceeds the size limit"));
        }
        Ok(())
    }

    /// Events without a `data` field (`: keepalive` comments, blank events)
    /// are not dispatched, per the SSE specification.
    pub(crate) fn next(&mut self) -> Result<Option<String>, Failure> {
        while let Some((end, separator)) = boundary(&self.bytes) {
            let event = self.bytes.drain(..end + separator).collect::<Vec<_>>();
            let data = event_data(&event[..end])?;
            if !data.is_empty() {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> Failure {
    Failure::new(CODE_INVALID_RESPONSE, message)
}

fn boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes.windows(2).position(|window| window == b"\n\n");
    let crlf = bytes.windows(4).position(|window| window == b"\r\n\r\n");
    [lf.map(|index| (index, 2)), crlf.map(|index| (index, 4))]
        .into_iter()
        .flatten()
        .min_by_key(|(index, _)| *index)
}

fn event_data(bytes: &[u8]) -> Result<String, Failure> {
    let event = std::str::from_utf8(bytes).map_err(|error| invalid(error.to_string()))?;
    let lines = event.lines().filter_map(|line| {
        line.strip_prefix("data:")
            .map(|data| data.strip_prefix(' ').unwrap_or(data))
    });
    Ok(lines.collect::<Vec<_>>().join("\n"))
}
