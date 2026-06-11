use zenocore::{Engine, Node, Scope, Value, Diagnostic};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod auth;
pub mod db;
pub mod http;
pub mod io;
pub mod proc;
pub mod proxy;
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

pub(crate) fn resolve_node_value(engine: &Engine, node: &Node, scope: &Arc<Scope>) -> Value {
    if let Some(ref val_str) = node.value {
        let dummy = Node {
            name: String::new(),
            value: Some(val_str.clone()),
            children: Vec::new(),
            line: node.line,
            col: node.col,
            filename: node.filename.clone(),
        };
        engine.resolve_shorthand_value(&dummy, scope)
    } else {
        Value::Nil
    }
}

pub(crate) fn value_to_serde_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Nil => serde_json::Value::Null,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into())),
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::List(l) => serde_json::Value::Array(l.iter().map(value_to_serde_json).collect()),
        Value::Map(m) => serde_json::Value::Object(m.iter().map(|(k, v)| (k.clone(), value_to_serde_json(v))).collect()),
    }
}

pub(crate) fn send_json_response(engine: &Engine, ctx: &mut zenocore::Context, status: u16, node: &Node, scope: &Arc<Scope>, success: bool) -> Result<(), Diagnostic> {
    let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
        Diagnostic {
            r#type: "error".to_string(),
            message: "http response helper: not in HTTP context".to_string(),
            filename: node.filename.clone(),
            line: node.line,
            col: node.col,
            slot: Some("http_response".to_string()),
        }
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
    response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), "application/json".to_string());
    *response_builder.body.lock().unwrap() = Some(body_str.into_bytes());

    Ok(())
}

pub fn register_custom_slots(engine: &mut Engine) {
    auth::register(engine);
    db::register(engine);
    http::register(engine);
    io::register(engine);
    proc::register(engine);
    proxy::register(engine);
    system::register(engine);
    util::register(engine);
}
