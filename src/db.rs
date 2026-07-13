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

fn get_container_data_dir() -> String {
    if let Ok(dir) = std::env::var("ZENO_CONTAINER_DATA_DIR") {
        return dir;
    }
    let default_path = "/var/lib/zeno-container";
    if std::path::Path::new(default_path).exists() {
        return default_path.to_string();
    }
    if let Ok(home) = std::env::var("HOME") {
        let fallback = format!("{}/.zeno-container", home);
        if std::path::Path::new(&fallback).exists() {
            return fallback;
        }
    }
    "./data/zeno-container".to_string()
}

fn resolve_container_ip_port(name: &str, host: &str, port: u16, default_port: u16) -> (String, u16) {
    if host == "127.0.0.1" || host == "localhost" {
        let data_dir = get_container_data_dir();
        let state_path = format!("{}/containers/{}/state.json", data_dir, name);
        if let Ok(content) = std::fs::read_to_string(&state_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(ip) = json["env"]["ZENO_IP"].as_str() {
                    let internal_port = if let Some(ports) = json["ports"].as_array() {
                        if let Some(port_str) = ports.get(0).and_then(|v| v.as_str()) {
                            port_str.split(':').last().and_then(|p| p.parse::<u16>().ok()).unwrap_or(default_port)
                        } else {
                            default_port
                        }
                    } else {
                        default_port
                    };
                    return (ip.to_string(), internal_port);
                }
            }
        }
    }
    (host.to_string(), port)
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
        
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect_with(options)
            .await?;
            
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
        let database_name = if database.is_empty() { "mysql" } else { database };
        
        use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
        use sqlx::mysql::MySqlPoolOptions;
        
        // 1. Try to connect directly to the user-specified host/port first
        let direct_options = MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .username(user)
            .password(password)
            .database(database_name)
            .ssl_mode(MySqlSslMode::Disabled);
            
        let direct_err = match MySqlPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_with(direct_options)
            .await
        {
            Ok(pool) => {
                self.pools.write().await.insert(name.to_string(), DbPool::MySql(pool));
                return Ok(());
            }
            Err(e) => {
                println!("[DB DEBUG] Direct connection to {}:{} failed: {}. Trying container IP resolution...", host, port, e);
                e
            }
        };
        
        // 2. If direct connection fails, fall back to container IP resolution
        let (resolved_host, resolved_port) = resolve_container_ip_port(name, host, port, 3306);
        if resolved_host != host || resolved_port != port {
            let container_options = MySqlConnectOptions::new()
                .host(&resolved_host)
                .port(resolved_port)
                .username(user)
                .password(password)
                .database(database_name)
                .ssl_mode(MySqlSslMode::Disabled);
                
            match MySqlPoolOptions::new()
                .acquire_timeout(std::time::Duration::from_secs(3))
                .connect_with(container_options)
                .await
            {
                Ok(pool) => {
                    self.pools.write().await.insert(name.to_string(), DbPool::MySql(pool));
                    return Ok(());
                }
                Err(e) => {
                    println!("[DB DEBUG] Container connection to {}:{} failed: {}", resolved_host, resolved_port, e);
                    return Err(e);
                }
            }
        }
        
        Err(direct_err)
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
        let database_name = if database.is_empty() { "postgres" } else { database };
        
        use sqlx::postgres::{PgConnectOptions, PgSslMode};
        use sqlx::postgres::PgPoolOptions;
        
        // 1. Try to connect directly to the user-specified host/port first
        let mut direct_options = PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(user)
            .password(password)
            .ssl_mode(PgSslMode::Disable);
        if !database.is_empty() {
            direct_options = direct_options.database(database_name);
        }
        
        let direct_err = match PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_with(direct_options)
            .await
        {
            Ok(pool) => {
                self.pools.write().await.insert(name.to_string(), DbPool::Postgres(pool));
                return Ok(());
            }
            Err(e) => {
                println!("[DB DEBUG] Direct connection to {}:{} failed: {}. Trying container IP resolution...", host, port, e);
                e
            }
        };
        
        // 2. If direct connection fails, fall back to container IP resolution
        let (resolved_host, resolved_port) = resolve_container_ip_port(name, host, port, 5432);
        if resolved_host != host || resolved_port != port {
            let mut container_options = PgConnectOptions::new()
                .host(&resolved_host)
                .port(resolved_port)
                .username(user)
                .password(password)
                .ssl_mode(PgSslMode::Disable);
            if !database.is_empty() {
                container_options = container_options.database(database_name);
            }
            
            match PgPoolOptions::new()
                .acquire_timeout(std::time::Duration::from_secs(3))
                .connect_with(container_options)
                .await
            {
                Ok(pool) => {
                    self.pools.write().await.insert(name.to_string(), DbPool::Postgres(pool));
                    return Ok(());
                }
                Err(e) => {
                    println!("[DB DEBUG] Container connection to {}:{} failed: {}", resolved_host, resolved_port, e);
                    return Err(e);
                }
            }
        }
        
        Err(direct_err)
    }

    pub async fn get_pool(&self, name: &str) -> Option<DbPool> {
        {
            let read = self.pools.read().await;
            if let Some(pool) = read.get(name) {
                return Some(pool.clone());
            }
        }

        if name == "default" {
            return None;
        }

        // On-demand self-healing loader from default SQLite DB
        let default_pool = match self.pools.read().await.get("default").cloned() {
            Some(DbPool::Sqlite(pool)) => pool,
            _ => return None,
        };

        match sqlx::query_as::<_, (String, String, i32, String, String)>(
            "SELECT driver, host, port, admin_user, admin_password FROM db_servers WHERE name = ?"
        )
        .bind(name)
        .fetch_optional(&default_pool)
        .await
        {
            Ok(Some((driver, host, port, user, password))) => {
                println!("[DB DEBUG] get_pool: Found registered server in DB: name={}, driver={}, host={}, port={}", name, driver, host, port);
                if driver == "mysql" {
                    if let Err(e) = self.add_mysql_connection(name, &host, port as u16, &user, &password, "mysql").await {
                        println!("[DB DEBUG] get_pool: Failed to add mysql connection: {}", e);
                    } else {
                        println!("[DB DEBUG] get_pool: Successfully added mysql connection pool: {}", name);
                    }
                } else if driver == "postgres" {
                    if let Err(e) = self.add_postgres_connection(name, &host, port as u16, &user, &password, "").await {
                        println!("[DB DEBUG] get_pool: Failed to add postgres connection: {}", e);
                    } else {
                        println!("[DB DEBUG] get_pool: Successfully added postgres connection pool: {}", name);
                    }
                }
                
                let read = self.pools.read().await;
                return read.get(name).cloned();
            }
            Ok(None) => {
                // If not found in db_servers, try to load from db_databases
                match sqlx::query_as::<_, (String, String, i32, String, String, String)>(
                    "SELECT s.driver, s.host, s.port, d.db_user, d.db_password, d.db_name \
                     FROM db_databases d \
                     JOIN db_servers s ON d.server_id = s.id \
                     WHERE d.db_name = ?"
                )
                .bind(name)
                .fetch_optional(&default_pool)
                .await
                {
                    Ok(Some((driver, host, port, user, password, db_name))) => {
                        println!("[DB DEBUG] get_pool: Found user database in DB: name={}, driver={}, host={}, port={}, db_name={}", name, driver, host, port, db_name);
                        if driver == "mysql" {
                            let _ = self.add_mysql_connection(name, &host, port as u16, &user, &password, &db_name).await;
                        } else if driver == "postgres" {
                            let _ = self.add_postgres_connection(name, &host, port as u16, &user, &password, &db_name).await;
                        }
                        
                        let read = self.pools.read().await;
                        return read.get(name).cloned();
                    }
                    _ => {
                        println!("[DB DEBUG] get_pool: No connection found in db_servers or db_databases with name: {}", name);
                    }
                }
            }
            Err(e) => {
                println!("[DB DEBUG] get_pool: Query failed: {}", e);
            }
        }

        None
    }
}

