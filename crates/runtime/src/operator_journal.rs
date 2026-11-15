use crate::operator_db::rows;
use crate::{DurableStore, Error, OperationId};
impl DurableStore {
    /// Explicit sensitive-data access. A page after an exclusive sequence; -1 starts at zero.
    /// # Errors
    /// Rejects invalid limits, unknown event versions and malformed payloads.
    pub async fn journal(
        &self,
        id: OperationId,
        after: i64,
        limit: u16,
    ) -> Result<Vec<(i64, agent::Event)>, Error> {
        if after < -1 || !(1..=100).contains(&limit) {
            return Err(Error::InvalidInput {
                field: "journal_page",
            });
        }
        let source = "SELECT sequence,schema_version,CAST(payload AS TEXT) FROM operation_events WHERE operation_id=$1 AND sequence>$2 ORDER BY sequence LIMIT $3";
        let records = rows!(
            self,
            source,
            (i64, i16, String),
            fetch_all,
            id.get(),
            after,
            i64::from(limit)
        )?;
        records.into_iter().map(decode).collect()
    }
}

fn decode((sequence, version, payload): (i64, i16, String)) -> Result<(i64, agent::Event), Error> {
    crate::stored_event::check_version(version)?;
    Ok((
        sequence,
        serde_json::from_str(&payload).map_err(Error::corrupt)?,
    ))
}
