use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;
use serde::{Serialize, Deserialize};

pub fn parse_host_port(host_str: &str) -> (String, Option<u16>) {
    let cleaned = host_str.trim();
    if cleaned.is_empty() {
        return (String::new(), None);
    }
    if let Some(pos) = cleaned.rfind(':') {
        let host = cleaned[..pos].to_string();
        if let Ok(port) = cleaned[pos + 1..].parse::<u16>() {
            return (host, Some(port));
        }
    }
    (cleaned.to_string(), None)
}

fn sanitize_host(host: &str) -> String {
    let mut cleaned = host.trim().to_string();
    if cleaned == "*" {
        return cleaned;
    }
    if let Some(pos) = cleaned.find("://") {
        cleaned = cleaned[pos + 3..].to_string();
    }
    if let Some(pos) = cleaned.find('/') {
        cleaned = cleaned[..pos].to_string();
    }
    cleaned
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRule {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub alternative_domain: String,
    pub path: String,
    pub target: String,
    pub strip_path: bool,
    pub enabled: bool,
    pub ssl_enabled: bool,
    pub ssl_status: String,
    pub managed_process_id: Option<String>,
    pub rule_type: String,
}

#[derive(Clone)]
pub struct ProxyManager {
    pool: SqlitePool,
    rules: Arc<RwLock<HashMap<String, ProxyRule>>>,
    rr_indices: Arc<tokio::sync::Mutex<HashMap<String, usize>>>,
    active_conns: Arc<std::sync::Mutex<HashMap<String, usize>>>,
    unhealthy_targets: Arc<RwLock<HashSet<String>>>,
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
        let alter_alternative_domain = "ALTER TABLE proxy_rules ADD COLUMN alternative_domain TEXT NOT NULL DEFAULT '';";
        let alter_rule_type = "ALTER TABLE proxy_rules ADD COLUMN rule_type TEXT NOT NULL DEFAULT 'proxy';";
        let _ = sqlx::query(alter_ssl_enabled).execute(&pool).await;
        let _ = sqlx::query(alter_ssl_status).execute(&pool).await;
        let _ = sqlx::query(alter_managed_process_id).execute(&pool).await;
        let _ = sqlx::query(alter_alternative_domain).execute(&pool).await;
        let _ = sqlx::query(alter_rule_type).execute(&pool).await;

        let active_conns = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let unhealthy_targets = Arc::new(RwLock::new(HashSet::new()));

        let unhealthy_clone = unhealthy_targets.clone();
        let pool_clone = pool.clone();

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build()
                .unwrap();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                // Query targets from DB (for currently active/enabled rules)
                let rows = match sqlx::query("SELECT target FROM proxy_rules WHERE enabled = 1")
                    .fetch_all(&pool_clone)
                    .await 
                {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let mut targets_to_check = HashSet::new();
                for row in rows {
                    use sqlx::Row;
                    if let Ok(target_str) = row.try_get::<String, _>("target") {
                        for t in target_str.split(',') {
                            let trimmed = t.trim();
                            if !trimmed.is_empty() {
                                targets_to_check.insert(trimmed.to_string());
                            }
                        }
                    }
                }

                for target in targets_to_check {
                    let url = if !target.starts_with("http://") && !target.starts_with("https://") {
                        format!("http://{}", target)
                    } else {
                        target.clone()
                    };

                    let is_healthy = match client.get(&url).send().await {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    };

                    let mut unhealthy = unhealthy_clone.write().await;
                    if is_healthy {
                        if unhealthy.remove(&target) {
                            println!("[Health Check] Target {} is now HEALTHY", target);
                        }
                    } else {
                        if unhealthy.insert(target.clone()) {
                            println!("[Health Check] Target {} is now UNHEALTHY", target);
                        }
                    }
                }
            }
        });

        Self {
            pool,
            rules: Arc::new(RwLock::new(HashMap::new())),
            rr_indices: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            active_conns,
            unhealthy_targets,
        }
    }

    pub async fn load_from_db(&self) -> Result<(), String> {
        let rows = sqlx::query("SELECT id, name, domain, alternative_domain, path, target, strip_path, enabled, ssl_enabled, ssl_status, managed_process_id, rule_type FROM proxy_rules")
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
            let alternative_domain: String = row.try_get("alternative_domain").unwrap_or_default();
            let path: String = row.get("path");
            let target: String = row.get("target");
            let strip_path_int: i32 = row.get("strip_path");
            let enabled_int: i32 = row.get("enabled");
            let ssl_enabled_int: i32 = row.get("ssl_enabled");
            let ssl_status: String = row.get("ssl_status");
            let managed_process_id: Option<String> = row.try_get("managed_process_id").ok();
            let rule_type: String = row.try_get("rule_type").unwrap_or_else(|_| "proxy".to_string());

            rules.insert(
                id.clone(),
                ProxyRule {
                    id,
                    name,
                    domain,
                    alternative_domain,
                    path,
                    target,
                    strip_path: strip_path_int != 0,
                    enabled: enabled_int != 0,
                    ssl_enabled: ssl_enabled_int != 0,
                    ssl_status,
                    managed_process_id,
                    rule_type,
                },
            );
        }
        Ok(())
    }

    pub async fn add_rule(
        &self,
        name: String,
        domain: String,
        alternative_domain: String,
        path: String,
        target: String,
        strip_path: bool,
        enabled: bool,
        ssl_enabled: bool,
        managed_process_id: Option<String>,
        rule_type: String,
    ) -> Result<String, String> {
        let id = format!("{:x}", rand::random::<u32>());
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };
        let ssl_enabled_int = if ssl_enabled { 1 } else { 0 };
        let ssl_status = if ssl_enabled { "pending".to_string() } else { "none".to_string() };

        let clean_domain = sanitize_host(&domain);
        let clean_alt_domain = sanitize_host(&alternative_domain);

        let mut clean_path = path.trim().to_string();
        if !clean_path.starts_with('/') {
            clean_path = format!("/{}", clean_path);
        }

        sqlx::query("INSERT INTO proxy_rules (id, name, domain, alternative_domain, path, target, strip_path, enabled, ssl_enabled, ssl_status, managed_process_id, rule_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&name)
            .bind(&clean_domain)
            .bind(&clean_alt_domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
            .bind(ssl_enabled_int)
            .bind(&ssl_status)
            .bind(&managed_process_id)
            .bind(&rule_type)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let rule = ProxyRule {
            id: id.clone(),
            name,
            domain: clean_domain,
            alternative_domain: clean_alt_domain,
            path: clean_path,
            target,
            strip_path,
            enabled,
            ssl_enabled,
            ssl_status,
            managed_process_id,
            rule_type,
        };

        self.rules.write().await.insert(id.clone(), rule);
        Ok(id)
    }

    pub async fn update_rule(
        &self,
        id: &str,
        name: String,
        domain: String,
        alternative_domain: String,
        path: String,
        target: String,
        strip_path: bool,
        enabled: bool,
        ssl_enabled: bool,
        managed_process_id: Option<String>,
        rule_type: String,
    ) -> Result<(), String> {
        let strip_path_int = if strip_path { 1 } else { 0 };
        let enabled_int = if enabled { 1 } else { 0 };
        let ssl_enabled_int = if ssl_enabled { 1 } else { 0 };

        let clean_domain = sanitize_host(&domain);
        let clean_alt_domain = sanitize_host(&alternative_domain);

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

        sqlx::query("UPDATE proxy_rules SET name = ?, domain = ?, alternative_domain = ?, path = ?, target = ?, strip_path = ?, enabled = ?, ssl_enabled = ?, ssl_status = ?, managed_process_id = ?, rule_type = ? WHERE id = ?")
            .bind(&name)
            .bind(&clean_domain)
            .bind(&clean_alt_domain)
            .bind(&clean_path)
            .bind(&target)
            .bind(strip_path_int)
            .bind(enabled_int)
            .bind(ssl_enabled_int)
            .bind(&new_status)
            .bind(&managed_process_id)
            .bind(&rule_type)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(rule) = self.rules.write().await.get_mut(id) {
            rule.name = name;
            rule.domain = clean_domain;
            rule.alternative_domain = clean_alt_domain;
            rule.path = clean_path;
            rule.target = target;
            rule.strip_path = strip_path;
            rule.enabled = enabled;
            rule.ssl_enabled = ssl_enabled;
            rule.ssl_status = new_status;
            rule.managed_process_id = managed_process_id;
            rule.rule_type = rule_type;
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

    pub async fn match_rule(&self, host: &str, request_port: u16, path: &str) -> Option<ProxyRule> {
        let rules = self.rules.read().await;
        let mut matched_rules: Vec<ProxyRule> = rules
            .values()
            .filter(|rule| {
                if !rule.enabled {
                    return false;
                }

                // Parse rule's domain and alternative_domain into (host, Option<port>)
                let (domain_host, domain_port) = parse_host_port(&rule.domain);
                let (alt_host, alt_port) = parse_host_port(&rule.alternative_domain);

                let domain_match = if rule.domain.is_empty() || rule.domain == "*" {
                    true
                } else {
                    let host_matches = domain_host.eq_ignore_ascii_case(host);
                    let port_matches = match domain_port {
                        Some(p) => p == request_port,
                        None => true, // Match any port if not specified
                    };
                    host_matches && port_matches
                };

                let alt_match = if rule.alternative_domain.is_empty() {
                    false
                } else {
                    let host_matches = alt_host.eq_ignore_ascii_case(host);
                    let port_matches = match alt_port {
                        Some(p) => p == request_port,
                        None => true, // Match any port if not specified
                    };
                    host_matches && port_matches
                };

                if !domain_match && !alt_match {
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

    pub fn increment_conn(&self, target: &str) {
        let mut conns = self.active_conns.lock().unwrap();
        let count = conns.entry(target.to_string()).or_insert(0);
        *count += 1;
    }

    pub fn decrement_conn(&self, target: &str) {
        let mut conns = self.active_conns.lock().unwrap();
        if let Some(count) = conns.get_mut(target) {
            if *count > 0 {
                *count -= 1;
            }
        }
    }

    pub async fn get_next_target(&self, _rule_id: &str, targets_str: &str) -> String {
        let targets: Vec<&str> = targets_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if targets.is_empty() {
            return String::new();
        }
        if targets.len() == 1 {
            return targets[0].to_string();
        }

        // Filter healthy targets
        let unhealthy = self.unhealthy_targets.read().await;
        let mut healthy_targets: Vec<&str> = targets
            .iter()
            .cloned()
            .filter(|t| !unhealthy.contains(*t))
            .collect();
        
        if healthy_targets.is_empty() {
            // Fallback to all targets if all are unhealthy
            healthy_targets = targets;
        }

        // Least Connections strategy: find target with minimum connections
        let conns = self.active_conns.lock().unwrap();
        let mut selected = healthy_targets[0].to_string();
        let mut min_conn = conns.get(&selected).cloned().unwrap_or(0);

        for target in healthy_targets.iter().skip(1) {
            let t_str = target.to_string();
            let c = conns.get(&t_str).cloned().unwrap_or(0);
            if c < min_conn {
                min_conn = c;
                selected = t_str;
            }
        }

        selected
    }
}

