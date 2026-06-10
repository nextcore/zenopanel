use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};
use zenocore::{Engine, Value, SlotMeta, Diagnostic};
use crate::slots::resolve_node_value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRule {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub path: String,
    pub target: String,
    pub strip_path: bool,
    pub enabled: bool,
    pub ssl_enabled: bool,
    pub ssl_status: String,
    pub managed_process_id: Option<String>,
}

#[derive(Clone)]
pub struct ProxyManager {
    pool: SqlitePool,
    rules: Arc<RwLock<HashMap<String, ProxyRule>>>,
}

impl ProxyManager {
    pub async fn new(pool: SqlitePool) -> Self {
        let create_table_query = "
            CREATE TABLE IF NOT EXISTS proxy_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                domain TEXT NOT NULL,
                path TEXT NOT NULL,
                target TEXT NOT NULL,
                strip_path INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1
            );
        ";
        if let Err(e) = sqlx::query(create_table_query).execute(&pool).await {
            eprintln!("Failed to create proxy_rules table: {}", e);
        }

        // Alter table to add SSL columns if they do not exist
        let alter_ssl_enabled = "ALTER TABLE proxy_rules ADD COLUMN ssl_enabled INTEGER NOT NULL DEFAULT 0;";
        let alter_ssl_status = "ALTER TABLE proxy_rules ADD COLUMN ssl_status TEXT NOT NULL DEFAULT 'none';";
        let alter_managed_process_id = "ALTER TABLE proxy_rules ADD COLUMN managed_process_id TEXT;";
        let _ = sqlx::query(alter_ssl_enabled).execute(&pool).await;
        let _ = sqlx::query(alter_ssl_status).execute(&pool).await;
        let _ = sqlx::query(alter_managed_process_id).execute(&pool).await;

        Self {
            pool,
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_from_db(&self) -> Result<(), String> {
        let rows = sqlx::query("SELECT id, name, domain, path, target, strip_path, enabled, ssl_enabled, ssl_status, managed_process_id FROM proxy_rules")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut rules = self.rules.write().await;
        rules.clear();
        for row in rows {
            use sqlx::Row;
            let id: String = row.get("id");
            let name: String = row.get("name");
            let domain: String = row.get("domain");
            let path: String = row.get("path");
            let target: String = row.get("target");
            let strip_path_int: i32 = row.get("strip_path");
            let enabled_int: i32 = row.get("enabled");
            let ssl_enabled_int: i32 = row.get("ssl_enabled");
            let ssl_status: String = row.get("ssl_status");
            let managed_process_id: Option<String> = row.try_get("managed_process_id").ok();

            rules.insert(
                id.clone(),
                ProxyRule {
                    id,
                    name,
                    domain,
                    path,
                    target,
                    strip_path: strip_path_int != 0,
                    enabled: enabled_int != 0,
                    ssl_enabled: ssl_enabled_int != 0,
                    ssl_status,
                    managed_process_id,
                },
            );
        }
        Ok(())
    }

    pub async fn add_rule(
        &self,
        name: String,
        domain: String,
        path: String,
        target: String,
        strip_path: bool,
        enabled: bool,
        ssl_enabled: bool,
        managed_process_id: Option<String>,
    ) -> Result<String, String> {
        let id = format!("{:x}", rand::random::<u32>());
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };
        let ssl_enabled_int = if ssl_enabled { 1 } else { 0 };
        let ssl_status = if ssl_enabled { "pending".to_string() } else { "none".to_string() };

        let mut clean_path = path.trim().to_string();
        if !clean_path.starts_with('/') {
            clean_path = format!("/{}", clean_path);
        }

        sqlx::query("INSERT INTO proxy_rules (id, name, domain, path, target, strip_path, enabled, ssl_enabled, ssl_status, managed_process_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&name)
            .bind(&domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
            .bind(ssl_enabled_int)
            .bind(&ssl_status)
            .bind(&managed_process_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let rule = ProxyRule {
            id: id.clone(),
            name,
            domain,
            path: clean_path,
            target,
            strip_path,
            enabled,
            ssl_enabled,
            ssl_status,
            managed_process_id,
        };

        self.rules.write().await.insert(id.clone(), rule);
        Ok(id)
    }

    pub async fn update_rule(
        &self,
        id: &str,
        name: String,
        domain: String,
        path: String,
        target: String,
        strip_path: bool,
        enabled: bool,
        ssl_enabled: bool,
        managed_process_id: Option<String>,
    ) -> Result<(), String> {
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };
        let ssl_enabled_int = if ssl_enabled { 1 } else { 0 };

        let mut clean_path = path.trim().to_string();
        if !clean_path.starts_with('/') {
            clean_path = format!("/{}", clean_path);
        }

        // Get existing rule to preserve ssl_status if ssl_enabled didn't change from true to true
        let existing = self.rules.read().await.get(id).cloned();
        let new_status = match existing {
            Some(old) => {
                if ssl_enabled {
                    if old.ssl_enabled { old.ssl_status } else { "pending".to_string() }
                } else {
                    "none".to_string()
                }
            }
            None => if ssl_enabled { "pending".to_string() } else { "none".to_string() }
        };

        sqlx::query("UPDATE proxy_rules SET name = ?, domain = ?, path = ?, target = ?, strip_path = ?, enabled = ?, ssl_enabled = ?, ssl_status = ?, managed_process_id = ? WHERE id = ?")
            .bind(&name)
            .bind(&domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
            .bind(ssl_enabled_int)
            .bind(&new_status)
            .bind(&managed_process_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(rule) = self.rules.write().await.get_mut(id) {
            rule.name = name;
            rule.domain = domain;
            rule.path = clean_path;
            rule.target = target;
            rule.strip_path = strip_path;
            rule.enabled = enabled;
            rule.ssl_enabled = ssl_enabled;
            rule.ssl_status = new_status;
            rule.managed_process_id = managed_process_id;
        }

        Ok(())
    }

    pub async fn update_ssl_status(&self, id: &str, status: &str) -> Result<(), String> {
        sqlx::query("UPDATE proxy_rules SET ssl_status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(rule) = self.rules.write().await.get_mut(id) {
            rule.ssl_status = status.to_string();
        }
        Ok(())
    }

    pub async fn remove_rule(&self, id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM proxy_rules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        self.rules.write().await.remove(id);
        Ok(())
    }

    pub async fn toggle_rule(&self, id: &str, enabled: bool) -> Result<(), String> {
        let enabled_int = if enabled { 1 } else { 0 };
        sqlx::query("UPDATE proxy_rules SET enabled = ? WHERE id = ?")
            .bind(enabled_int)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(rule) = self.rules.write().await.get_mut(id) {
            rule.enabled = enabled;
        }
        Ok(())
    }

    pub async fn list_rules(&self) -> Vec<ProxyRule> {
        let rules = self.rules.read().await;
        rules.values().cloned().collect()
    }

    pub async fn match_rule(&self, host: &str, path: &str) -> Option<ProxyRule> {
        let rules = self.rules.read().await;
        let mut matched_rules: Vec<ProxyRule> = rules
            .values()
            .filter(|rule| {
                if !rule.enabled {
                    return false;
                }

                // Match domain: exact match, wildcard *, or empty
                let domain_match = rule.domain.is_empty() 
                    || rule.domain == "*" 
                    || rule.domain.eq_ignore_ascii_case(host);

                if !domain_match {
                    return false;
                }

                // Match path prefix: /api prefix matches /api, /api/, /api/v1
                let path_match = if rule.path == "/" {
                    true
                } else {
                    path == rule.path || path.starts_with(&format!("{}/", rule.path.trim_end_matches('/')))
                };

                path_match
            })
            .cloned()
            .collect();

        // Sort: specific domain matches first, then longer path prefixes
        matched_rules.sort_by(|a, b| {
            let a_is_specific_domain = !a.domain.is_empty() && a.domain != "*";
            let b_is_specific_domain = !b.domain.is_empty() && b.domain != "*";

            match (a_is_specific_domain, b_is_specific_domain) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    b.path.len().cmp(&a.path.len())
                }
            }
        });

        matched_rules.into_iter().next()
    }
}

fn get_cert_details(domain: &str, ssl_status: &str) -> (String, String, i64) {
    let certs_dir = "./certs";
    let cert_path = format!("{}/{}.crt", certs_dir, domain);
    
    if std::path::Path::new(&cert_path).exists() {
        if let Ok(metadata) = std::fs::metadata(&cert_path) {
            if let Ok(modified) = metadata.modified() {
                let validity_days = if ssl_status == "active_letsencrypt" { 90 } else { 365 };
                let duration = std::time::Duration::from_secs(validity_days * 24 * 3600);
                if let Some(expires_at) = modified.checked_add(duration) {
                    let now = std::time::SystemTime::now();
                    
                    let days_remaining = match expires_at.duration_since(now) {
                        Ok(d) => (d.as_secs() / 86400) as i64,
                        Err(_) => 0,
                    };
                    
                    let expires_chrono: chrono::DateTime<chrono::Local> = expires_at.into();
                    let expiration_str = expires_chrono.format("%Y-%m-%d %H:%M:%S").to_string();
                    
                    let issuer = if ssl_status == "active_letsencrypt" {
                        "Let's Encrypt".to_string()
                    } else {
                        "Self-Signed".to_string()
                    };
                    
                    return (issuer, expiration_str, days_remaining);
                }
            }
        }
    }
    ("None".to_string(), "N/A".to_string(), 0)
}

fn proxy_rule_to_value(rule: &ProxyRule) -> Value {
    let mut map = HashMap::new();
    map.insert("id".to_string(), Value::String(rule.id.clone()));
    map.insert("name".to_string(), Value::String(rule.name.clone()));
    map.insert("domain".to_string(), Value::String(rule.domain.clone()));
    map.insert("path".to_string(), Value::String(rule.path.clone()));
    map.insert("target".to_string(), Value::String(rule.target.clone()));
    map.insert("strip_path".to_string(), Value::Bool(rule.strip_path));
    map.insert("enabled".to_string(), Value::Bool(rule.enabled));
    map.insert("ssl_enabled".to_string(), Value::Bool(rule.ssl_enabled));
    map.insert("ssl_status".to_string(), Value::String(rule.ssl_status.clone()));
    map.insert("managed_process_id".to_string(), Value::String(rule.managed_process_id.clone().unwrap_or_default()));

    let (issuer, expiry, days) = get_cert_details(&rule.domain, &rule.ssl_status);
    map.insert("ssl_issuer".to_string(), Value::String(issuer));
    map.insert("ssl_expiration".to_string(), Value::String(expiry));
    map.insert("ssl_days_remaining".to_string(), Value::Int(days));

    Value::Map(map)
}

pub fn register_proxy_slots(engine: &mut Engine) {
    // 1. proxy.list
    engine.register(
        "proxy.list",
        Arc::new(|_engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<ProxyManager>>("proxy_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proxy.list: ProxyManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proxy.list".to_string()),
                }
            })?;

            let mut target = "proxies".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let list_fut = pm.list_rules();
            let list = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(list_fut)
            });

            let val_list = Value::List(list.iter().map(proxy_rule_to_value).collect());
            scope.set(&target, val_list);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // 2. proxy.add
    engine.register(
        "proxy.add",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<ProxyManager>>("proxy_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proxy.add: ProxyManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proxy.add".to_string()),
                }
            })?;

            let mut name = String::new();
            let mut domain = String::new();
            let mut path = "/".to_string();
            let mut target_url = String::new();
            let mut strip_path = false;
            let mut enabled = true;
            let mut ssl_enabled = false;
            let mut managed_process_id = None;
            let mut target = "id".to_string();

            if node.value.is_some() {
                name = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "name" {
                    name = val.to_string_coerce();
                } else if child.name == "domain" {
                    domain = val.to_string_coerce();
                } else if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "target" {
                    target_url = val.to_string_coerce();
                } else if child.name == "strip_path" {
                    strip_path = val.to_bool();
                } else if child.name == "enabled" {
                    enabled = val.to_bool();
                } else if child.name == "ssl_enabled" {
                    ssl_enabled = val.to_bool();
                } else if child.name == "managed_process_id" {
                    let s = val.to_string_coerce();
                    if !s.is_empty() {
                        managed_process_id = Some(s);
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let add_fut = pm.add_rule(name, domain, path, target_url, strip_path, enabled, ssl_enabled, managed_process_id);
            let id = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(add_fut)
            }).map_err(|e| Diagnostic {
                r#type: "error".to_string(),
                message: format!("proxy.add failed: {}", e),
                filename: node.filename.clone(),
                line: node.line,
                col: node.col,
                slot: Some("proxy.add".to_string()),
            })?;

            scope.set(&target, Value::String(id));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // 3. proxy.update
    engine.register(
        "proxy.update",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<ProxyManager>>("proxy_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proxy.update: ProxyManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proxy.update".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut name = String::new();
            let mut domain = String::new();
            let mut path = "/".to_string();
            let mut target_url = String::new();
            let mut strip_path = false;
            let mut enabled = true;
            let mut ssl_enabled = false;
            let mut managed_process_id = None;
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "name" {
                    name = val.to_string_coerce();
                } else if child.name == "domain" {
                    domain = val.to_string_coerce();
                } else if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "target" {
                    target_url = val.to_string_coerce();
                } else if child.name == "strip_path" {
                    strip_path = val.to_bool();
                } else if child.name == "enabled" {
                    enabled = val.to_bool();
                } else if child.name == "ssl_enabled" {
                    ssl_enabled = val.to_bool();
                } else if child.name == "managed_process_id" {
                    let s = val.to_string_coerce();
                    if !s.is_empty() {
                        managed_process_id = Some(s);
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let update_fut = pm.update_rule(&id, name, domain, path, target_url, strip_path, enabled, ssl_enabled, managed_process_id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(update_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // 4. proxy.delete
    engine.register(
        "proxy.delete",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<ProxyManager>>("proxy_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proxy.delete: ProxyManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proxy.delete".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let delete_fut = pm.remove_rule(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(delete_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // 5. proxy.toggle
    engine.register(
        "proxy.toggle",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<ProxyManager>>("proxy_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proxy.toggle: ProxyManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proxy.toggle".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut enabled = true;
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "enabled" {
                    enabled = val.to_bool();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let toggle_fut = pm.toggle_rule(&id, enabled);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(toggle_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}
