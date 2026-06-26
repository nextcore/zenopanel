use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zenocore::{Diagnostic, Engine, Node, Scope, Value};

pub mod auth;
pub mod box_slot;
pub mod db;
pub mod http;
pub mod io;
pub mod proc;
pub mod proxy;
pub mod service;
pub mod system;
pub mod util;

static FUNCTION_REGISTRY: Mutex<Option<HashMap<String, Node>>> = Mutex::new(None);

pub(crate) fn register_function(name: String, node: Node) {
    let mut lock = FUNCTION_REGISTRY.lock().unwrap();
    if lock.is_none() {
        *lock = Some(HashMap::new());
    }
    lock.as_mut().unwrap().insert(name, node);
}

pub(crate) fn get_function(name: &str) -> Option<Node> {
    let lock = FUNCTION_REGISTRY.lock().unwrap();
    lock.as_ref().and_then(|map| map.get(name).cloned())
}

pub struct HttpResponseBuilder {
    pub status: Mutex<u16>,
    pub headers: Mutex<HashMap<String, String>>,
    pub cookies: Mutex<Vec<String>>,
    pub body: Mutex<Option<Vec<u8>>>,
}

fn resolve_scope_path(path: &str, scope: &Arc<Scope>) -> Option<Value> {
    if path.contains('.') {
        let parts: Vec<&str> = path.split('.').collect();
        if let Some(mut current) = scope.get(parts[0]) {
            for part in &parts[1..] {
                match current {
                    Value::Map(ref m) => {
                        if let Some(next_val) = m.get(*part) {
                            current = next_val.clone();
                        } else {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            Some(current)
        } else {
            None
        }
    } else {
        scope.get(path)
    }
}

fn interpolate_string(s: &str, scope: &Arc<Scope>) -> String {
    let mut current = s.to_string();
    let mut iterations = 0;
    while iterations < 10 && current.contains("${") {
        let mut result = String::new();
        let mut last_idx = 0;
        let mut replaced = false;
        while let Some(start_idx) = current[last_idx..].find("${") {
            let absolute_start = last_idx + start_idx;
            result.push_str(&current[last_idx..absolute_start]);
            if let Some(end_idx) = current[absolute_start..].find('}') {
                let absolute_end = absolute_start + end_idx;
                let key = current[absolute_start + 2..absolute_end].trim();
                let clean_key = if key.starts_with('$') { &key[1..] } else { key };
                if let Some(val) = resolve_scope_path(clean_key, scope) {
                    result.push_str(&val.to_string_coerce());
                }
                last_idx = absolute_end + 1;
                replaced = true;
            } else {
                break;
            }
        }
        result.push_str(&current[last_idx..]);
        current = result;
        if !replaced {
            break;
        }
        iterations += 1;
    }
    current
}

pub(crate) fn resolve_node_value(engine: &Engine, node: &Node, scope: &Arc<Scope>) -> Value {
    let raw_val = if let Some(ref val_str) = node.value {
        let val_str = val_str.trim();
        if val_str.starts_with('$') {
            let key = &val_str[1..];
            if key.contains('.') {
                let parts: Vec<&str> = key.split('.').collect();
                if let Some(mut current) = scope.get(parts[0]) {
                    let mut found = true;
                    for part in &parts[1..] {
                        match current {
                            Value::Map(ref m) => {
                                if let Some(next_val) = m.get(*part) {
                                    current = next_val.clone();
                                } else {
                                    found = false;
                                    break;
                                }
                            }
                            _ => {
                                found = false;
                                break;
                            }
                        }
                    }
                    if found {
                        current
                    } else {
                        Value::Nil
                    }
                } else {
                    Value::Nil
                }
            } else {
                let dummy = Node {
                    name: String::new(),
                    value: Some(val_str.to_string()),
                    children: Vec::new(),
                    line: node.line,
                    col: node.col,
                    filename: node.filename.clone(),
                };
                engine.resolve_shorthand_value(&dummy, scope)
            }
        } else {
            let dummy = Node {
                name: String::new(),
                value: Some(val_str.to_string()),
                children: Vec::new(),
                line: node.line,
                col: node.col,
                filename: node.filename.clone(),
            };
            engine.resolve_shorthand_value(&dummy, scope)
        }
    } else {
        Value::Nil
    };

    match raw_val {
        Value::String(s) => {
            Value::String(interpolate_string(&s, scope))
        }
        other => other,
    }
}

pub(crate) fn value_to_serde_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Nil => serde_json::Value::Null,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => {
            serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()))
        }
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(l) => serde_json::Value::Array(l.iter().map(value_to_serde_json).collect()),
        Value::Map(m) => serde_json::Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), value_to_serde_json(v)))
                .collect(),
        ),
    }
}

pub(crate) fn send_json_response(
    engine: &Engine,
    ctx: &mut zenocore::Context,
    status: u16,
    node: &Node,
    scope: &Arc<Scope>,
    success: bool,
) -> Result<(), Diagnostic> {
    let response_builder = ctx
        .get::<HttpResponseBuilder>("response_builder")
        .ok_or_else(|| Diagnostic {
            r#type: "error".to_string(),
            message: "http response helper: not in HTTP context".to_string(),
            filename: node.filename.clone(),
            line: node.line,
            col: node.col,
            slot: Some("http_response".to_string()),
        })?;

    let mut map = HashMap::new();
    map.insert("success".to_string(), Value::Bool(success));
    for child in &node.children {
        let val = engine.resolve_shorthand_value(child, scope);
        map.insert(child.name.clone(), val);
    }

    let json_val = value_to_serde_json(&Value::Map(map));
    let body_str = serde_json::to_string(&json_val).unwrap_or_default();

    *response_builder.status.lock().unwrap() = status;
    response_builder
        .headers
        .lock()
        .unwrap()
        .insert("Content-Type".to_string(), "application/json".to_string());
    *response_builder.body.lock().unwrap() = Some(body_str.into_bytes());

    Ok(())
}

pub fn register_custom_slots(engine: &mut Engine) {
    auth::register(engine);
    box_slot::register(engine);
    db::register(engine);
    http::register(engine);
    io::register(engine);
    proc::register(engine);
    proxy::register(engine);
    system::register(engine);
    util::register(engine);
    service::register(engine);
}
