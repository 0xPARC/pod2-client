use anyhow::{Context, Result};
use deadpool_sqlite::{Config, Pool, Runtime};
use include_dir::{include_dir, Dir};
use lazy_static::lazy_static;
use log::info;
use rusqlite_migration::Migrations;
use uuid::Uuid;

pub mod store;

lazy_static! {
    pub static ref MIGRATIONS_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/migrations");
    pub static ref MIGRATIONS: Migrations<'static> =
        Migrations::from_directory(&MIGRATIONS_DIR).unwrap();
}

pub type ConnectionPool = Pool;

#[derive(Clone)]
pub struct Db {
    pool: ConnectionPool,
}

impl Db {
    /// Creates a new `Db` instance.
    ///
    /// This will:
    /// 1. Create a `deadpool-sqlite` connection pool.
    /// 2. Run the provided database migrations.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the SQLite database file, or `None` for an in-memory database.
    /// * `migrations` - A `rusqlite_migration::Migrations` object.
    pub async fn new(path: Option<&str>, migrations: &'static Migrations<'static>) -> Result<Self> {
        let db_path = match path {
            Some(p) => p.to_string(),
            None => {
                // Use a uniquely named, shared-cache in-memory database for each pool instance
                // when no path is specified. This ensures test isolation.
                let unique_name = Uuid::new_v4().to_string();
                format!("file:{unique_name}?mode=memory&cache=shared")
            }
        };
        info!("Initializing database with path: {db_path}");

        let config = Config::new(db_path);
        let pool = config.create_pool(Runtime::Tokio1)?;

        let conn = pool
            .get()
            .await
            .context("Failed to get connection for migrations")?;

        conn.interact(move |conn| migrations.to_latest(conn))
            .await
            .map_err(|e| anyhow::anyhow!("InteractError during migration: {e}"))
            .context("Failed to run migrations")??;

        info!("Migrations applied successfully.");

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &ConnectionPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    use super::*;

    fn check_table_exists(
        conn: &mut Connection,
        table_name: &str,
    ) -> Result<bool, rusqlite::Error> {
        conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name= ?1",
            [table_name],
            |_| Ok(true),
        )
        .map(|_| true)
        .or_else(|err| {
            if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                Ok(false) // Table doesn't exist
            } else {
                Err(err) // Other DB error
            }
        })
    }

    #[tokio::test]
    async fn test_db_new_in_memory() {
        let db = Db::new(None, &MIGRATIONS)
            .await
            .expect("Failed to initialize in-memory DB");

        let conn = db
            .pool()
            .get()
            .await
            .expect("Failed to get connection from pool");
        let exists_result = conn
            .interact(|conn| check_table_exists(conn, "pods"))
            .await
            .expect("Interaction failed for pods table");
        let pods_exists = exists_result.expect("check_table_exists for pods failed");
        assert!(
            pods_exists,
            "'pods' table should exist in in-memory DB after schema creation"
        );

        let spaces_exists_result = conn
            .interact(|conn| check_table_exists(conn, "spaces"))
            .await
            .expect("Interaction failed for spaces table");
        let spaces_exists = spaces_exists_result.expect("check_table_exists for spaces failed");
        assert!(
            spaces_exists,
            "'spaces' table should exist in in-memory DB after schema creation"
        );
    }

    #[tokio::test]
    async fn test_db_new_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path_str = temp_file.path().to_str().unwrap();

        {
            let db = Db::new(Some(path_str), &MIGRATIONS)
                .await
                .expect("Failed to initialize file DB");

            let conn = db
                .pool()
                .get()
                .await
                .expect("Failed to get connection from pool");
            let exists_result = conn
                .interact(|conn| check_table_exists(conn, "pods"))
                .await
                .expect("Interaction failed for pods table");
            let pods_exists = exists_result.expect("check_table_exists for pods failed");
            assert!(
                pods_exists,
                "'pods' table should exist in file DB after schema creation"
            );

            let spaces_exists_result = conn
                .interact(|conn| check_table_exists(conn, "spaces"))
                .await
                .expect("Interaction failed for spaces table");
            let spaces_exists = spaces_exists_result.expect("check_table_exists for spaces failed");
            assert!(
                spaces_exists,
                "'spaces' table should exist in file DB after schema creation"
            );
            // Pool drops here, manager should handle connection closure
        }

        // Re-open the connection normally to check persistence (pool manager handles actual file)
        let mut conn2 = Connection::open(path_str).expect("Failed to reopen DB file");

        assert!(
            check_table_exists(&mut conn2, "pods").expect("check_table_exists for pods failed"),
            "'pods' table should persist in file DB",
        );
        assert!(
            check_table_exists(&mut conn2, "spaces").expect("check_table_exists for spaces failed"),
            "'spaces' table should persist in file DB",
        );
    }
}
