//! Grouping of lane-wide attachment rows by owning operation, shared by
//! every attachment kind: group, parse each owner's rows against its
//! declared count, and reject orphans that no operation claims.

use std::collections::HashMap;

use crate::Error;

/// One owner's rows parsed against the count that owner declared.
type ParseGroup<R, T> = fn(Vec<R>, i64, i16) -> Result<Vec<T>, Error>;

pub(crate) fn group<R, T>(
    counts: Vec<(i64, i16)>,
    rows: Vec<R>,
    owner: fn(&R) -> i64,
    parse: ParseGroup<R, T>,
) -> Result<HashMap<i64, Vec<T>>, Error> {
    let mut grouped: HashMap<i64, Vec<R>> = HashMap::new();
    for row in rows {
        grouped.entry(owner(&row)).or_default().push(row);
    }
    let mut parsed = HashMap::new();
    for (id, expected) in counts {
        let rows = grouped.remove(&id).unwrap_or_default();
        parsed.insert(id, parse(rows, id, expected)?);
    }
    if !grouped.is_empty() {
        return Err(Error::CorruptRecord {
            message: "orphan attachment rows".to_owned(),
        });
    }
    Ok(parsed)
}
