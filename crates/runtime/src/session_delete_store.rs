// Both backends use one ownership graph; only transaction syntax and SQL dialect differ.
macro_rules! backend {
    ($module:ident, $store:ty, $connection:ty, $error:path, $begin:literal,
     $now:literal, $lock:literal, $sessions:literal) => {
        pub(crate) mod $module {
            use crate::{Address, Error, session_delete_sql as queries};
            pub(crate) async fn delete(store: &$store, address: &Address) -> Result<bool, Error> {
                let mut tx = store.pool.begin_with($begin).await.map_err($error)?;
                let Some(root) = root(&mut tx, address).await? else {
                    return Ok(false);
                };
                let sessions = tree(&mut tx, root).await?;
                let busy: Vec<bool> =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(sql(queries::DELIVERIES)))
                        .bind(&sessions)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err($error)?;
                if busy.contains(&true) {
                    return Err(Error::SessionBusy);
                }
                remove(&mut tx, &sessions).await?;
                tx.commit().await.map_err($error)?;
                Ok(true)
            }
            async fn remove(connection: &mut $connection, sessions: &str) -> Result<(), Error> {
                for statement in queries::statements() {
                    sqlx::query(sqlx::AssertSqlSafe(sql(&statement)))
                        .bind(sessions)
                        .execute(&mut *connection)
                        .await
                        .map_err($error)?;
                }
                Ok(())
            }
            async fn root(
                connection: &mut $connection,
                address: &Address,
            ) -> Result<Option<i64>, Error> {
                let row: Option<(i64, bool)> =
                    sqlx::query_as(sqlx::AssertSqlSafe(sql(queries::ROOT)))
                        .bind(address.surface())
                        .bind(address.conversation())
                        .fetch_optional(connection)
                        .await
                        .map_err($error)?;
                match row {
                    Some((_, true)) => Err(Error::InvalidInput {
                        field: "root_session",
                    }),
                    Some((id, false)) => Ok(Some(id)),
                    None => Ok(None),
                }
            }
            async fn tree(connection: &mut $connection, root: i64) -> Result<String, Error> {
                let rows: Vec<(i64, bool)> =
                    sqlx::query_as(sqlx::AssertSqlSafe(sql(queries::TREE)))
                        .bind(root)
                        .fetch_all(connection)
                        .await
                        .map_err($error)?;
                if rows.iter().any(|row| row.1) {
                    return Err(Error::SessionBusy);
                }
                serde_json::to_string(&rows.into_iter().map(|row| row.0).collect::<Vec<_>>())
                    .map_err(Error::corrupt)
            }
            fn sql(source: &str) -> String {
                source
                    .replace("@now", $now)
                    .replace("@lock", $lock)
                    .replace("@sessions", $sessions)
            }
        }
    };
}
pub(crate) use backend;
