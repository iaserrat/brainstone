//! Transactional application effect ledger shared semantically by every adapter.
use crate::protocol::Error;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

pub fn record(path: &str, key: &str, args: &Value) -> Result<Value, Error> {
    let value = argument(args)?;
    let mut connection = open(path)?;
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute("INSERT INTO attempts VALUES(?1,?2)", params![key, value])?;
    let previous: Option<String> = transaction
        .query_row(
            "SELECT value FROM effects WHERE retry_key=?1",
            [key],
            |row| row.get(0),
        )
        .optional()?;
    if previous.as_deref().is_some_and(|stored| stored != value) {
        return Err("effect_idempotency_conflict".into());
    }
    transaction.execute(
        "INSERT OR IGNORE INTO effects VALUES(?1,?2)",
        params![key, value],
    )?;
    transaction.commit()?;
    Ok(json!({"value":value}))
}

fn open(path: &str) -> Result<Connection, Error> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS effects(retry_key TEXT PRIMARY KEY,value TEXT);
        CREATE TABLE IF NOT EXISTS attempts(retry_key TEXT,value TEXT);",
    )?;
    Ok(connection)
}

fn argument(args: &Value) -> Result<&str, Error> {
    let object = args.as_object().ok_or("invalid_arguments")?;
    if object.len() != 1 {
        return Err("invalid_arguments".into());
    }
    object
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "invalid_arguments".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_same_retry_key_applies_one_effect_and_preserves_its_value() {
        let path = std::env::temp_dir().join(format!(
            "agent-rs-eval-ledger-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let path_text = path.to_str().unwrap();
        let args = json!({"value":"Zoë ☕"});
        assert_eq!(record(path_text, "retry", &args).unwrap(), args);
        assert_eq!(record(path_text, "retry", &args).unwrap(), args);
        assert!(record(path_text, "retry", &json!({"value":"changed"})).is_err());
        let connection = Connection::open(&path).unwrap();
        let effects: i64 = connection
            .query_row("SELECT count(*) FROM effects", [], |row| row.get(0))
            .unwrap();
        let attempts: i64 = connection
            .query_row("SELECT count(*) FROM attempts", [], |row| row.get(0))
            .unwrap();
        assert_eq!((effects, attempts), (1, 2));
        drop(connection);
        std::fs::remove_file(path).unwrap();
    }
}
