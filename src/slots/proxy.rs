use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use std::sync::Arc;
use std::collections::HashMap;
use crate::proxyman::{ProxyManager, ProxyRule};

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
    map.insert("alternative_domain".to_string(), Value::String(rule.alternative_domain.clone()));
    map.insert("path".to_string(), Value::String(rule.path.clone()));
    map.insert("target".to_string(), Value::String(rule.target.clone()));
    map.insert("strip_path".to_string(), Value::Bool(rule.strip_path));
    map.insert("enabled".to_string(), Value::Bool(rule.enabled));
    map.insert("ssl_enabled".to_string(), Value::Bool(rule.ssl_enabled));
    map.insert("ssl_status".to_string(), Value::String(rule.ssl_status.clone()));
    map.insert("managed_process_id".to_string(), Value::String(rule.managed_process_id.clone().unwrap_or_default()));
    map.insert("rule_type".to_string(), Value::String(rule.rule_type.clone()));

    let (issuer, expiry, days) = get_cert_details(&rule.domain, &rule.ssl_status);
    map.insert("ssl_issuer".to_string(), Value::String(issuer));
    map.insert("ssl_expiration".to_string(), Value::String(expiry));
    map.insert("ssl_days_remaining".to_string(), Value::Int(days));

    Value::Map(map)
}

pub fn register(engine: &mut Engine) {
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
            let mut alternative_domain = String::new();
            let mut path = "/".to_string();
            let mut target_url = String::new();
            let mut strip_path = false;
            let mut enabled = true;
            let mut ssl_enabled = false;
            let mut managed_process_id = None;
            let mut rule_type = "proxy".to_string();
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
                } else if child.name == "alternative_domain" {
                    alternative_domain = val.to_string_coerce();
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
                } else if child.name == "rule_type" {
                    rule_type = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let add_fut = pm.add_rule(name, domain, alternative_domain, path, target_url, strip_path, enabled, ssl_enabled, managed_process_id, rule_type);
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
            let mut alternative_domain = String::new();
            let mut path = "/".to_string();
            let mut target_url = String::new();
            let mut strip_path = false;
            let mut enabled = true;
            let mut ssl_enabled = false;
            let mut managed_process_id = None;
            let mut rule_type = "proxy".to_string();
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
                } else if child.name == "alternative_domain" {
                    alternative_domain = val.to_string_coerce();
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
                } else if child.name == "rule_type" {
                    rule_type = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let update_fut = pm.update_rule(&id, name, domain, alternative_domain, path, target_url, strip_path, enabled, ssl_enabled, managed_process_id, rule_type);
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
