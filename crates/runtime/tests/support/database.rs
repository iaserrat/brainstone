use sqlx::PgPool;

pub(crate) struct Database {
    pub url: String,
    _directory: Option<tempfile::TempDir>,
    admin: Option<PgPool>,
    schema: String,
}

impl Database {
    pub(crate) fn sqlite() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", directory.path().join("runtime.db").display());
        Self {
            url,
            _directory: Some(directory),
            admin: None,
            schema: String::new(),
        }
    }

    pub(crate) async fn postgres() -> Self {
        let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("dedicated test database");
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let schema = format!("backend_contract_{suffix}");
        let admin = PgPool::connect(&url).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&admin)
            .await
            .unwrap();
        let separator = if url.contains('?') { '&' } else { '?' };
        Self {
            url: format!("{url}{separator}options=-csearch_path%3D{schema}"),
            _directory: None,
            admin: Some(admin),
            schema,
        }
    }

    pub(crate) async fn close(self) {
        if let Some(admin) = self.admin {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP SCHEMA {} CASCADE",
                self.schema
            )))
            .execute(&admin)
            .await
            .unwrap();
            admin.close().await;
        }
    }
}
