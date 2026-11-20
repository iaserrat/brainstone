use crate::postgres_backend::postgres_migrations;
use sqlx::migrate::Migrate;
use sqlx::{PgConnection, PgPool};

use crate::Error;

/// A live index rebuilt outside any transaction: `CONCURRENTLY` keeps writes
/// flowing while the replacement builds.
struct Repair {
    invalid: &'static str,
    drop: &'static str,
    create: &'static str,
}

const REPRESENTATIVE: &str = "CREATE INDEX CONCURRENTLY IF NOT EXISTS \
    operations_delivery_representative \
    ON operations (lane_id, (delivery_target->>'adapter'), id) \
    WHERE state IN ('accepted', 'running')";

const CLAIMABLE: &str = "CREATE INDEX CONCURRENTLY IF NOT EXISTS \
    operation_deliveries_claimable \
    ON operation_deliveries (operation_id) \
    WHERE NOT settled";

const REPAIRS: [Repair; 2] = [
    Repair {
        invalid: "SELECT COALESCE((SELECT NOT indisvalid FROM pg_index \
            WHERE indexrelid = to_regclass('operations_delivery_representative')), FALSE)",
        drop: "DROP INDEX CONCURRENTLY IF EXISTS operations_delivery_representative",
        create: REPRESENTATIVE,
    },
    Repair {
        invalid: "SELECT COALESCE((SELECT NOT indisvalid FROM pg_index \
            WHERE indexrelid = to_regclass('operation_deliveries_claimable')), FALSE)",
        drop: "DROP INDEX CONCURRENTLY IF EXISTS operation_deliveries_claimable",
        create: CLAIMABLE,
    },
];

pub(super) async fn migrate(pool: &PgPool) -> Result<(), Error> {
    let mut connection = pool.acquire().await.map_err(Error::store)?;
    connection.close_on_drop();
    connection.lock().await.map_err(Error::migration)?;
    let migration = migrate_locked(&mut connection).await;
    let unlock = connection.unlock().await.map_err(Error::migration);
    migration?;
    unlock?;
    Ok(())
}

async fn migrate_locked(connection: &mut PgConnection) -> Result<(), Error> {
    repair_locked(connection).await?;
    let mut migrator = postgres_migrations::migrator();
    migrator.set_locking(false);
    migrator
        .run_direct(None, connection, false)
        .await
        .map_err(Error::migration)
}

async fn repair_locked(connection: &mut PgConnection) -> Result<(), Error> {
    for repair in &REPAIRS {
        if invalid(connection, repair).await? {
            rebuild(connection, repair).await?;
        }
    }
    Ok(())
}

async fn invalid(connection: &mut PgConnection, repair: &Repair) -> Result<bool, Error> {
    sqlx::query_scalar(repair.invalid)
        .fetch_one(&mut *connection)
        .await
        .map_err(Error::store)
}

async fn rebuild(connection: &mut PgConnection, repair: &Repair) -> Result<(), Error> {
    sqlx::query(repair.drop)
        .execute(&mut *connection)
        .await
        .map_err(Error::store)?;
    sqlx::raw_sql(repair.create)
        .execute(connection)
        .await
        .map_err(Error::store)?;
    Ok(())
}
