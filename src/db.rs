use sqlx::{SqlitePool, MySqlPool, PgPool};
use std::collections::HashMap;
use tokio::sync::RwLock;

use std::sync::Arc;

#[derive(Clone)]
pub enum DbPool {
    Sqlite(SqlitePool),
    MySql(MySqlPool),
    Postgres(PgPool),
}

#[derive(Clone)]
pub struct DBManager {
    pub pools: Arc<RwLock<HashMap<String, DbPool>>>,
}

impl DBManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_sqlite_connection(&self, name: &str, dsn: &str) -> Result<(), sqlx::Error> {
        let mut clean_dsn = dsn.to_string();
        if clean_dsn.starts_with("./") {
            clean_dsn = clean_dsn[2..].to_string();
        }
        
        let path_str = clean_dsn.strip_prefix("sqlite:").unwrap_or(&clean_dsn).to_string();
        if let Some(parent) = std::path::Path::new(&path_str).parent() {
            if parent.as_os_str() != "" && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path_str))?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;
        self.pools.write().await.insert(name.to_string(), DbPool::Sqlite(pool));
        Ok(())
    }

    pub async fn add_mysql_connection(
        &self,
        name: &str,
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        database: &str,
    ) -> Result<(), sqlx::Error> {
        use sqlx::mysql::MySqlConnectOptions;
        let mut options = MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .username(user)
            .password(password);
        if !database.is_empty() {
            options = options.database(database);
        }
        let pool = MySqlPool::connect_with(options).await?;
        self.pools.write().await.insert(name.to_string(), DbPool::MySql(pool));
        Ok(())
    }

    pub async fn add_postgres_connection(
        &self,
        name: &str,
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        database: &str,
    ) -> Result<(), sqlx::Error> {
        use sqlx::postgres::PgConnectOptions;
        let mut options = PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(user)
            .password(password);
        if !database.is_empty() {
            options = options.database(database);
        }
        let pool = PgPool::connect_with(options).await?;
        self.pools.write().await.insert(name.to_string(), DbPool::Postgres(pool));
        Ok(())
    }

    pub async fn get_pool(&self, name: &str) -> Option<DbPool> {
        self.pools.read().await.get(name).cloned()
    }
}

