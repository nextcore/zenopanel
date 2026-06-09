use sqlx::SqlitePool;
use std::collections::HashMap;
use tokio::sync::RwLock;

use std::sync::Arc;

#[derive(Clone)]
pub enum DbPool {
    Sqlite(SqlitePool),
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

        let pool = SqlitePool::connect(&format!("sqlite:{}", path_str)).await?;
        self.pools.write().await.insert(name.to_string(), DbPool::Sqlite(pool));
        Ok(())
    }

    pub async fn get_pool(&self, name: &str) -> Option<DbPool> {
        self.pools.read().await.get(name).cloned()
    }
}
