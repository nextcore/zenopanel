use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};
use zenocore::{Engine, Node, Scope, Value, SlotMeta, Diagnostic};
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

        Self {
            pool,
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_from_db(&self) -> Result<(), String> {
        let rows = sqlx::query("SELECT id, name, domain, path, target, strip_path, enabled FROM proxy_rules")
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
    ) -> Result<String, String> {
        let id = format!("{:x}", rand::random::<u32>());
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };

        let mut clean_path = path.trim().to_string();
        if !clean_path.starts_with('/') {
            clean_path = format!("/{}", clean_path);
        }

        sqlx::query("INSERT INTO proxy_rules (id, name, domain, path, target, strip_path, enabled) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&name)
            .bind(&domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
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
    ) -> Result<(), String> {
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };

        let mut clean_path = path.trim().to_string();
        if !clean_path.starts_with('/') {
            clean_path = format!("/{}", clean_path);
        }

        sqlx::query("UPDATE proxy_rules SET name = ?, domain = ?, path = ?, target = ?, strip_path = ?, enabled = ? WHERE id = ?")
            .bind(&name)
            .bind(&domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
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

fn proxy_rule_to_value(rule: &ProxyRule) -> Value {
    let mut map = HashMap::new();
    map.insert("id".to_string(), Value::String(rule.id.clone()));
    map.insert("name".to_string(), Value::String(rule.name.clone()));
    map.insert("domain".to_string(), Value::String(rule.domain.clone()));
    map.insert("path".to_string(), Value::String(rule.path.clone()));
    map.insert("target".to_string(), Value::String(rule.target.clone()));
    map.insert("strip_path".to_string(), Value::Bool(rule.strip_path));
    map.insert("enabled".to_string(), Value::Bool(rule.enabled));
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
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let add_fut = pm.add_rule(name, domain, path, target_url, strip_path, enabled);
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
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let update_fut = pm.update_rule(&id, name, domain, path, target_url, strip_path, enabled);
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
