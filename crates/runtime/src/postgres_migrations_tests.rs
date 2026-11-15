//! The Postgres schema ships as one baseline migration with no history to
//! rewrite. These properties keep it that way: a fresh database applies it in
//! a single transaction, and no legacy backfill ever returns.

#[test]
fn the_schema_is_one_transactional_baseline() {
    let migrator = super::migrator();
    assert_eq!(migrator.migrations.len(), 1);
    let migration = &migrator.migrations[0];
    assert_eq!(migration.version, 1);
    assert!(!migration.no_tx);
}

#[test]
fn the_baseline_creates_instead_of_rewriting_history() {
    let migrator = super::migrator();
    let sql = migrator.migrations[0].sql.as_str();
    assert!(!sql.contains("UPDATE "));
    assert!(!sql.contains("ADD COLUMN"));
    assert!(!sql.contains("CONCURRENTLY"));
    assert!(!sql.contains("slack"));
}
