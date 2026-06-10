use zenocore::{Engine, Node, Scope, SlotMeta, Diagnostic, Value};
use crate::db::DBManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sysinfo::{System, Disks, Networks, Pid};

static FUNCTION_REGISTRY: Mutex<Option<HashMap<String, Node>>> = Mutex::new(None);

fn register_function(name: String, node: Node) {
    let mut lock = FUNCTION_REGISTRY.lock().unwrap();
    if lock.is_none() {
        *lock = Some(HashMap::new());
    }
    lock.as_mut().unwrap().insert(name, node);
}

fn get_function(name: &str) -> Option<Node> {
    let lock = FUNCTION_REGISTRY.lock().unwrap();
    lock.as_ref().and_then(|map| map.get(name).cloned())
}

fn zip_dir_recursive(
    src_dir: &std::path::Path,
    current_dir: &std::path::Path,
    zip: &mut zip::ZipWriter<std::fs::File>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            zip_dir_recursive(src_dir, &path, zip)?;
        } else {
            let name = path.strip_prefix(src_dir)?;
            zip.start_file(name.to_string_lossy(), zip::write::FileOptions::default())?;
            let mut f = std::fs::File::open(&path)?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

fn zip_file(src_file: &std::path::Path, zip: &mut zip::ZipWriter<std::fs::File>) -> Result<(), Box<dyn std::error::Error>> {
    let name = src_file.file_name().ok_or("Invalid file name")?;
    zip.start_file(name.to_string_lossy(), zip::write::FileOptions::default())?;
    let mut f = std::fs::File::open(src_file)?;
    std::io::copy(&mut f, zip)?;
    Ok(())
}


pub struct HttpResponseBuilder {
    pub status: Mutex<u16>,
    pub headers: Mutex<HashMap<String, String>>,
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

fn value_to_serde_json(val: &Value) -> serde_json::Value {
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

fn send_json_response(engine: &Engine, ctx: &mut zenocore::Context, status: u16, node: &Node, scope: &Arc<Scope>, success: bool) -> Result<(), Diagnostic> {
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

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    if days > 0 {
        format!("{} days, {} hours, {} mins", days, hours, minutes)
    } else if hours > 0 {
        format!("{} hours, {} mins", hours, minutes)
    } else {
        format!("{} mins", minutes)
    }
}

pub fn register_custom_slots(engine: &mut Engine) {
    engine.register(
        "cast.to_int",
        Arc::new(|engine, _ctx, node, scope| {
            let mut val = Value::Nil;
            let mut target = "cast_result".to_string();

            if node.value.is_some() {
                val = resolve_node_value(engine, node, scope);
            }

            for child in &node.children {
                let child_val = engine.resolve_shorthand_value(child, scope);
                if child.name == "val" || child.name == "value" {
                    val = child_val;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let int_val = val.to_int();
            scope.set(&target, Value::Int(int_val));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "coalesce",
        Arc::new(|engine, _ctx, node, scope| {
            let mut val = Value::Nil;
            let mut def = Value::Nil;
            let mut target = "coalesce_result".to_string();

            if node.value.is_some() {
                val = resolve_node_value(engine, node, scope);
            }

            for child in &node.children {
                let child_val = engine.resolve_shorthand_value(child, scope);
                if child.name == "val" || child.name == "value" {
                    val = child_val;
                } else if child.name == "default" || child.name == "def" {
                    def = child_val;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let is_nil = match &val {
                Value::Nil => true,
                Value::String(s) => s.is_empty() || s == "nil" || s == "<nil>" || s.starts_with('$'),
                _ => false,
            };

            let result = if is_nil { def.clone() } else { val.clone() };
            scope.set(&target, result);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "fn",
        Arc::new(|engine, _ctx, node, scope| {
            let func_name = resolve_node_value(engine, node, scope).to_string_coerce();
            if func_name.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "fn: function name is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("fn".to_string()),
                });
            }

            register_function(func_name, node.clone());
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "call",
        Arc::new(|engine, ctx, node, scope| {
            let func_name = resolve_node_value(engine, node, scope).to_string_coerce();
            if func_name.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "call: function name is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("call".to_string()),
                });
            }

            let func_node = get_function(&func_name).ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("call: function '{}' not found", func_name),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("call".to_string()),
                }
            })?;

            for child in &func_node.children {
                engine.execute(ctx, child, scope)?;
            }

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "include",
        Arc::new(|engine, ctx, node, scope| {
            let path = resolve_node_value(engine, node, scope).to_string_coerce();
            let content = std::fs::read_to_string(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("include failed to read file '{}': {}", path, e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("include".to_string()),
                }
            })?;
            
            let parsed_node = zenocore::parser::parse_string(&content, &path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("include failed to parse file '{}': {:?}", path, e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("include".to_string()),
                }
            })?;

            engine.execute(ctx, &parsed_node, scope)
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.archive",
        Arc::new(|engine, _ctx, node, scope| {
            let mut source = "".to_string();
            let mut dest = "".to_string();
            let mut target = "archive_result".to_string();

            if node.value.is_some() {
                source = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" || child.name == "src" {
                    source = val.to_string_coerce();
                } else if child.name == "dest" || child.name == "dst" {
                    dest = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if source.is_empty() || dest.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.archive: both source (path) and dest paths are required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.archive".to_string()),
                });
            }

            let src_path = std::path::Path::new(&source);
            let dst_path = std::path::Path::new(&dest);

            if let Some(parent) = dst_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.archive failed to create target parent dir: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.archive".to_string()),
                        }
                    })?;
                }
            }

            let file = std::fs::File::create(&dst_path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.archive failed to create destination file: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.archive".to_string()),
                }
            })?;

            let mut zip = zip::ZipWriter::new(file);

            let res = if src_path.is_dir() {
                zip_dir_recursive(src_path, src_path, &mut zip)
            } else {
                zip_file(src_path, &mut zip)
            };

            res.map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.archive failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.archive".to_string()),
                }
            })?;

            zip.finish().map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.archive failed to finalize zip: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.archive".to_string()),
                }
            })?;

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.extract",
        Arc::new(|engine, _ctx, node, scope| {
            let mut source = "".to_string();
            let mut dest = "".to_string();
            let mut target = "extract_result".to_string();

            if node.value.is_some() {
                source = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" || child.name == "src" {
                    source = val.to_string_coerce();
                } else if child.name == "dest" || child.name == "dst" {
                    dest = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if source.is_empty() || dest.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.extract: both source (path) and dest paths are required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.extract".to_string()),
                });
            }

            let file = std::fs::File::open(&source).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.extract failed to open source file: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.extract".to_string()),
                }
            })?;

            let mut archive = zip::ZipArchive::new(file).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.extract failed to read zip archive: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.extract".to_string()),
                }
            })?;

            let target_dir = std::path::Path::new(&dest);
            if !target_dir.exists() {
                std::fs::create_dir_all(target_dir).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.extract failed to create target directory: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.extract".to_string()),
                    }
                })?;
            }

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.extract failed to read zip entry: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.extract".to_string()),
                    }
                })?;

                let outpath = match file.enclosed_name() {
                    Some(path) => target_dir.join(path),
                    None => continue,
                };

                if (*file.name()).ends_with('/') {
                    std::fs::create_dir_all(&outpath).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.extract failed to create entry directory: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.extract".to_string()),
                        }
                    })?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(&p).map_err(|e| {
                                Diagnostic {
                                    r#type: "error".to_string(),
                                    message: format!("io.file.extract failed to create entry parent directory: {}", e),
                                    filename: node.filename.clone(),
                                    line: node.line,
                                    col: node.col,
                                    slot: Some("io.file.extract".to_string()),
                                }
                            })?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.extract failed to create entry file: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.extract".to_string()),
                        }
                    })?;
                    std::io::copy(&mut file, &mut outfile).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.extract failed to copy file contents: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.extract".to_string()),
                        }
                    })?;
                }

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
                    }
                }
            }

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ==========================================
    // 1. HTTP SLOTS
    // ==========================================
    engine.register(
        "http.response",
        Arc::new(|engine, ctx, node, scope| {
            let mut status = 200;
            let mut content_type = "application/json".to_string();
            let mut body = Value::Nil;

            if node.value.is_some() {
                let val = resolve_node_value(engine, node, scope);
                status = val.to_int() as u16;
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "status" {
                    status = val.to_int() as u16;
                } else if child.name == "type" {
                    content_type = val.to_string_coerce();
                } else if child.name == "data" || child.name == "body" {
                    body = val;
                }
            }

            let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.response: not in HTTP context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.response".to_string()),
                }
            })?;

            let body_bytes = if content_type == "application/json" {
                let json_val = value_to_serde_json(&body);
                serde_json::to_string(&json_val).unwrap_or_default().into_bytes()
            } else {
                body.to_string_coerce().into_bytes()
            };

            *response_builder.status.lock().unwrap() = status;
            response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), content_type);
            *response_builder.body.lock().unwrap() = Some(body_bytes);
            Ok(())
        }),
        SlotMeta {
            description: "Send HTTP response".to_string(),
            example: "http.response: 200".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "any".to_string(),
        },
    );

    engine.register(
        "http.query",
        Arc::new(|engine, ctx, node, scope| {
            let query_name = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut target = query_name.clone();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let val = if let Some(query_params) = ctx.get::<HashMap<String, String>>("query_params") {
                query_params.get(&query_name).cloned().unwrap_or_default()
            } else {
                String::new()
            };

            scope.set(&target, Value::String(val));
            Ok(())
        }),
        SlotMeta {
            description: "Get query parameter".to_string(),
            example: "http.query: 'id'".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "string".to_string(),
        },
    );

    engine.register(
        "http.json_body",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "input".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let val = ctx.get::<Value>("json_body").map(|v| (*v).clone()).unwrap_or(Value::Nil);
            scope.set(&target, val);
            Ok(())
        }),
        SlotMeta {
            description: "Get JSON request body".to_string(),
            example: "http.json_body { as: $body }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "any".to_string(),
        },
    );

    // Response Helpers
    engine.register("http.ok", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 200, node, scope, true)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.created", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 201, node, scope, true)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.accepted", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 202, node, scope, true)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.bad_request", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 400, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.unauthorized", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 401, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.forbidden", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 403, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.not_found", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 404, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.validation_error", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 422, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });
    engine.register("http.server_error", Arc::new(|engine, ctx, node, scope| send_json_response(engine, ctx, 500, node, scope, false)), SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() });

    engine.register(
        "debug.print",
        Arc::new(|engine, _ctx, node, scope| {
            let val = resolve_node_value(engine, node, scope);
            println!("DEBUG.PRINT: {:?}", val);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ==========================================
    // 2. FILESYSTEM SLOTS
    // ==========================================
    engine.register(
        "io.file.read",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = String::new();
            let mut target = "file_content".to_string();

            if node.value.is_some() {
                path = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let content = std::fs::read_to_string(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.read failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.read".to_string()),
                }
            })?;

            scope.set(&target, Value::String(content));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.write",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = String::new();
            let mut content = String::new();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "content" {
                    content = val.to_string_coerce();
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.write: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.write".to_string()),
                });
            }

            if std::env::var("APP_ENV").unwrap_or_default() != "development" {
                let path_lower = path.to_lowercase();
                if path_lower.ends_with(".zl") || path_lower.ends_with(".rs") || path_lower.ends_with(".env") || path_lower.contains(".git") {
                    return Err(Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("security violation: modifying sensitive file '{}' is restricted in production", path),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.write".to_string()),
                    });
                }
            }

            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            std::fs::write(&path, content).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.write failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.write".to_string()),
                }
            })?;
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.dir.create",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = resolve_node_value(engine, node, scope).to_string_coerce();
            if path.is_empty() {
                for child in &node.children {
                    if child.name == "path" {
                        path = engine.resolve_shorthand_value(child, scope).to_string_coerce();
                    }
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.dir.create: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.dir.create".to_string()),
                });
            }

            std::fs::create_dir_all(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.dir.create failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.dir.create".to_string()),
                }
            })?;
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.delete",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = resolve_node_value(engine, node, scope).to_string_coerce();
            if path.is_empty() {
                for child in &node.children {
                    if child.name == "path" {
                        path = engine.resolve_shorthand_value(child, scope).to_string_coerce();
                    }
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.delete: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.delete".to_string()),
                });
            }

            let p = std::path::Path::new(&path);
            if p.exists() {
                if p.is_dir() {
                    std::fs::remove_dir_all(p).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.delete failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.delete".to_string()),
                        }
                    })?;
                } else {
                    std::fs::remove_file(p).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.delete failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.delete".to_string()),
                        }
                    })?;
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ==========================================
    // 3. DATABASE SLOTS
    // ==========================================
    engine.register(
        "db.select",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.select: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.select".to_string()),
                }
            })?;

            let mut db_name = "default".to_string();
            let mut query_sql = String::new();
            let mut bind_args = Vec::new();
            let mut target = "rows".to_string();
            let mut only_first = false;

            if node.value.is_some() {
                query_sql = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "sql" {
                    query_sql = val.to_string_coerce();
                } else if child.name == "db" || child.name == "connection" {
                    db_name = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                } else if child.name == "first" {
                    only_first = val.to_bool();
                } else if child.name == "bind" {
                    for bind_child in &child.children {
                        let bind_val = engine.resolve_shorthand_value(bind_child, scope);
                        bind_args.push(bind_val);
                    }
                }
            }

            let results = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool_opt = db_mgr.get_pool(&db_name).await;
                    if let Some(crate::db::DbPool::Sqlite(pool)) = pool_opt {
                        let mut query = sqlx::query(&query_sql);
                        for arg in bind_args {
                            query = match arg {
                                Value::Nil => query.bind(None::<String>),
                                Value::String(s) => query.bind(s),
                                Value::Int(i) => query.bind(i),
                                Value::Float(f) => query.bind(f),
                                Value::Bool(b) => query.bind(b),
                                _ => query.bind(arg.to_string_coerce()),
                            };
                        }
                        
                        let rows = query.fetch_all(&pool).await.map_err(|e| e.to_string())?;
                        let mut res_list = Vec::new();
                        use sqlx::{Row, Column, TypeInfo};
                        for row in rows {
                            let mut map = HashMap::new();
                            for col in row.columns() {
                                let col_name = col.name().to_string();
                                let val = match col.type_info().name() {
                                    "INTEGER" | "INT" | "BIGINT" => {
                                        if let Ok(v) = row.try_get::<i64, _>(col.ordinal()) {
                                            Value::Int(v)
                                        } else {
                                            Value::Nil
                                        }
                                    }
                                    "REAL" | "DOUBLE" | "FLOAT" => {
                                        if let Ok(v) = row.try_get::<f64, _>(col.ordinal()) {
                                            Value::Float(v)
                                        } else {
                                            Value::Nil
                                        }
                                    }
                                    "BOOLEAN" | "BOOL" => {
                                        if let Ok(v) = row.try_get::<bool, _>(col.ordinal()) {
                                            Value::Bool(v)
                                        } else {
                                            Value::Nil
                                        }
                                    }
                                    _ => {
                                        if let Ok(v) = row.try_get::<String, _>(col.ordinal()) {
                                            Value::String(v)
                                        } else {
                                            Value::Nil
                                        }
                                    }
                                };
                                map.insert(col_name, val);
                            }
                            res_list.push(Value::Map(map));
                        }
                        Ok::<_, String>(res_list)
                    } else {
                        Err(format!("database connection '{}' not found", db_name))
                    }
                })
            }).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: e,
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.select".to_string()),
                }
            })?;

            if only_first {
                let first_val = results.into_iter().next().unwrap_or(Value::Nil);
                scope.set(&target, first_val);
            } else {
                scope.set(&target, Value::List(results));
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.execute",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.execute: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.execute".to_string()),
                }
            })?;

            let mut db_name = "default".to_string();
            let mut query_sql = String::new();
            let mut bind_args = Vec::new();

            if node.value.is_some() {
                query_sql = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "sql" {
                    query_sql = val.to_string_coerce();
                } else if child.name == "db" || child.name == "connection" {
                    db_name = val.to_string_coerce();
                } else if child.name == "bind" {
                    for bind_child in &child.children {
                        let bind_val = engine.resolve_shorthand_value(bind_child, scope);
                        bind_args.push(bind_val);
                    }
                }
            }

            let (affected, last_id) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool_opt = db_mgr.get_pool(&db_name).await;
                    if let Some(crate::db::DbPool::Sqlite(pool)) = pool_opt {
                        let mut query = sqlx::query(&query_sql);
                        for arg in bind_args {
                            query = match arg {
                                Value::Nil => query.bind(None::<String>),
                                Value::String(s) => query.bind(s),
                                Value::Int(i) => query.bind(i),
                                Value::Float(f) => query.bind(f),
                                Value::Bool(b) => query.bind(b),
                                _ => query.bind(arg.to_string_coerce()),
                            };
                        }
                        
                        let res = query.execute(&pool).await.map_err(|e| e.to_string())?;
                        Ok::<_, String>((res.rows_affected() as i64, res.last_insert_rowid()))
                    } else {
                        Err(format!("database connection '{}' not found", db_name))
                    }
                })
            }).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: e,
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.execute".to_string()),
                }
            })?;

            scope.set("db_affected", Value::Int(affected));
            scope.set("db_last_id", Value::Int(last_id));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ==========================================
    // 4. SYSTEM SLOTS
    // ==========================================
    engine.register(
        "system.info",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "sys_info".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let mut sys = System::new_all();
            sys.refresh_all();

            let hostname = System::host_name().unwrap_or_default();
            let cores = sys.cpus().len() as i64;
            let uptime_sec = System::uptime();
            let uptime = format_uptime(uptime_sec);
            let os_ver = System::long_os_version().unwrap_or_default();
            let cpu_model = sys.cpus().first().map(|cpu| cpu.brand().to_string()).unwrap_or_default();
            let platform = std::env::consts::OS.to_string();
            let arch = std::env::consts::ARCH.to_string();

            let mut info = HashMap::new();
            info.insert("hostname".to_string(), Value::String(hostname));
            info.insert("cores".to_string(), Value::Int(cores));
            info.insert("uptime".to_string(), Value::String(uptime));
            info.insert("os".to_string(), Value::String(os_ver));
            info.insert("cpu_model".to_string(), Value::String(cpu_model));
            info.insert("platform".to_string(), Value::String(platform));
            info.insert("arch".to_string(), Value::String(arch));

            scope.set(&target, Value::Map(info));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.stats",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "sys_stats".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let mut sys = System::new_all();
            sys.refresh_cpu_usage();
            std::thread::sleep(std::time::Duration::from_millis(100));
            sys.refresh_cpu_usage();

            let cpu = sys.global_cpu_info().cpu_usage() as f64;
            let mem_total = sys.total_memory() as f64;
            let mem_free = sys.free_memory() as f64;
            let mem_used = sys.used_memory() as f64;
            let mem_pct = if mem_total > 0.0 { (mem_used / mem_total) * 100.0 } else { 0.0 };

            let mut disk_total = 0.0;
            let mut disk_free = 0.0;
            let mut disk_used = 0.0;
            let mut disk_pct = 0.0;

            let disks = Disks::new_with_refreshed_list();
            if let Some(disk) = disks.iter().find(|d| d.mount_point() == std::path::Path::new("/")) {
                disk_total = disk.total_space() as f64;
                disk_free = disk.available_space() as f64;
                disk_used = disk_total - disk_free;
                if disk_total > 0.0 {
                    disk_pct = (disk_used / disk_total) * 100.0;
                }
            }

            let networks = Networks::new_with_refreshed_list();
            let mut net_rx = 0.0;
            let mut net_tx = 0.0;
            for (_interface_name, network) in &networks {
                net_rx += network.total_received() as f64;
                net_tx += network.total_transmitted() as f64;
            }

            let mut stats = HashMap::new();
            stats.insert("cpu".to_string(), Value::Float(cpu));
            stats.insert("mem_total".to_string(), Value::Float(mem_total));
            stats.insert("mem_free".to_string(), Value::Float(mem_free));
            stats.insert("mem_avail".to_string(), Value::Float(mem_free)); // fallback
            stats.insert("mem_used".to_string(), Value::Float(mem_used));
            stats.insert("mem_pct".to_string(), Value::Float(mem_pct));
            stats.insert("disk_total".to_string(), Value::Float(disk_total));
            stats.insert("disk_free".to_string(), Value::Float(disk_free));
            stats.insert("disk_used".to_string(), Value::Float(disk_used));
            stats.insert("disk_pct".to_string(), Value::Float(disk_pct));
            stats.insert("net_rx".to_string(), Value::Float(net_rx));
            stats.insert("net_tx".to_string(), Value::Float(net_tx));

            scope.set(&target, Value::Map(stats));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.processes",
        Arc::new(|engine, _ctx, node, scope| {
            let mut target = "sys_processes".to_string();
            let mut sort_by = "mem".to_string();
            let mut limit = 50;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                } else if child.name == "sort" {
                    sort_by = val.to_string_coerce();
                } else if child.name == "limit" {
                    limit = val.to_int() as usize;
                }
            }

            let mut sys = System::new_all();
            sys.refresh_all();

            let mut procs: Vec<Value> = sys.processes().iter().map(|(pid, proc)| {
                let mut map = HashMap::new();
                map.insert("pid".to_string(), Value::Int(pid.as_u32() as i64));
                map.insert("name".to_string(), Value::String(proc.name().to_string()));
                map.insert("cpu".to_string(), Value::Float(proc.cpu_usage() as f64));
                
                let mem_bytes = proc.memory() as f64;
                let mem_pct = if sys.total_memory() > 0 {
                    (mem_bytes / sys.total_memory() as f64) * 100.0
                } else {
                    0.0
                };
                map.insert("memory".to_string(), Value::Float(mem_pct));
                map.insert("status".to_string(), Value::String(format!("{:?}", proc.status())));
                Value::Map(map)
            }).collect();

            if sort_by == "cpu" {
                procs.sort_by(|a, b| {
                    let a_val = a.to_map().get("cpu").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    let b_val = b.to_map().get("cpu").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    b_val.partial_cmp(&a_val).unwrap_or(std::cmp::Ordering::Equal)
                });
            } else {
                procs.sort_by(|a, b| {
                    let a_val = a.to_map().get("memory").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    let b_val = b.to_map().get("memory").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    b_val.partial_cmp(&a_val).unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            if procs.len() > limit {
                procs.truncate(limit);
            }

            scope.set(&target, Value::List(procs));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.kill",
        Arc::new(|engine, _ctx, node, scope| {
            let mut pid = 0;
            let mut target = "kill_success".to_string();

            if node.value.is_some() {
                pid = resolve_node_value(engine, node, scope).to_int();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "pid" {
                    pid = val.to_int();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if pid <= 0 {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "system.kill: invalid pid".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.kill".to_string()),
                });
            }

            let sys = System::new_all();
            let sys_pid = Pid::from(pid as usize);
            let found = sys.process(sys_pid).is_some();
            let success = if let Some(proc) = sys.process(sys_pid) {
                proc.kill()
            } else {
                false
            };

            println!("SYSTEM.KILL: pid={}, found={}, success={}", pid, found, success);

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.port_check",
        Arc::new(|engine, _ctx, node, scope| {
            let mut port = 0;
            let mut target = "port_info".to_string();

            if node.value.is_some() {
                port = resolve_node_value(engine, node, scope).to_int();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "port" {
                    port = val.to_int();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if port <= 0 || port > 65535 {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "system.port_check: invalid port number".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.port_check".to_string()),
                });
            }

            let mut res = HashMap::new();
            res.insert("port".to_string(), Value::Int(port));

            let output = std::process::Command::new("lsof")
                .args(&["-i", &format!(":{}", port), "-s", "tcp:listen", "-F", "pc"])
                .output();

            let mut in_use = false;
            let mut pid = None;
            let mut name = None;

            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        if line.starts_with('p') {
                            if let Ok(p) = line[1..].parse::<i64>() {
                                pid = Some(p);
                                in_use = true;
                            }
                        } else if line.starts_with('c') {
                            name = Some(line[1..].to_string());
                        }
                    }
                }
            }

            res.insert("in_use".to_string(), Value::Bool(in_use));
            res.insert("pid".to_string(), match pid {
                Some(p) => Value::Int(p),
                None => Value::Nil,
            });
            res.insert("process_name".to_string(), match name {
                Some(n) => Value::String(n),
                None => Value::Nil,
            });

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.dir_list",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = ".".to_string();
            let mut target = "dir_list".to_string();

            if node.value.is_some() {
                path = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let entries = std::fs::read_dir(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("system.dir_list failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.dir_list".to_string()),
                }
            })?;

            let mut list = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(meta) = entry.metadata() {
                        let mut map = HashMap::new();
                        map.insert("name".to_string(), Value::String(entry.file_name().to_string_lossy().into_owned()));
                        map.insert("is_dir".to_string(), Value::Bool(meta.is_dir()));
                        map.insert("size".to_string(), Value::Int(meta.len() as i64));
                        
                        let mod_time = meta.modified().ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| {
                                let datetime: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()).unwrap_or_default();
                                datetime.to_rfc3339()
                            })
                            .unwrap_or_default();
                        map.insert("mod_time".to_string(), Value::String(mod_time));
                        
                        #[cfg(unix)]
                        let mode = {
                            use std::os::unix::fs::PermissionsExt;
                            format!("{:o}", meta.permissions().mode())
                        };
                        #[cfg(not(unix))]
                        let mode = format!("{:o}", meta.permissions().readonly() as u32);
                        
                        map.insert("mode".to_string(), Value::String(mode));
                        list.push(Value::Map(map));
                    }
                }
            }

            list.sort_by(|a, b| {
                let a_map = a.to_map();
                let b_map = b.to_map();
                let a_is_dir = a_map.get("is_dir").cloned().unwrap_or(Value::Bool(false)).to_bool();
                let b_is_dir = b_map.get("is_dir").cloned().unwrap_or(Value::Bool(false)).to_bool();
                if a_is_dir != b_is_dir {
                    b_is_dir.cmp(&a_is_dir)
                } else {
                    let a_name = a_map.get("name").cloned().unwrap_or(Value::Nil).to_string_coerce();
                    let b_name = b_map.get("name").cloned().unwrap_or(Value::Nil).to_string_coerce();
                    a_name.cmp(&b_name)
                }
            });

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.env",
        Arc::new(|engine, _ctx, node, scope| {
            let env_name = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut target = "env_val".to_string();

            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let val = std::env::var(&env_name).unwrap_or_default();
            scope.set(&target, Value::String(val));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_status",
        Arc::new(|engine, _ctx, node, scope| {
            let mut service = String::new();
            let mut target = "service_status".to_string();

            if node.value.is_some() {
                service = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "service" {
                    service = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let output = std::process::Command::new("systemctl")
                .args(&["is-active", &service])
                .output();

            let (status, active) = match output {
                Ok(out) => {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let act = s == "active";
                    (s, act)
                }
                Err(_) => ("unknown".to_string(), false)
            };

            let mut res = HashMap::new();
            res.insert("service".to_string(), Value::String(service));
            res.insert("status".to_string(), Value::String(status));
            res.insert("active".to_string(), Value::Bool(active));

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_control",
        Arc::new(|engine, _ctx, node, scope| {
            let mut service = String::new();
            let mut action = String::new();
            let mut target = "control_result".to_string();

            if node.value.is_some() {
                service = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "service" {
                    service = val.to_string_coerce();
                } else if child.name == "action" {
                    action = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let valid_actions = ["start", "stop", "restart", "reload", "enable", "disable"];
            if !valid_actions.contains(&action.as_str()) {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("system.service_control: invalid action '{}'", action),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.service_control".to_string()),
                });
            }

            let output = std::process::Command::new("systemctl")
                .args(&[&action, &service])
                .output();

            let mut res = HashMap::new();
            match output {
                Ok(out) => {
                    let success = out.status.success();
                    res.insert("success".to_string(), Value::Bool(success));
                    if !success {
                        let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        res.insert("error".to_string(), Value::String(err_msg));
                    } else {
                        res.insert("error".to_string(), Value::String(String::new()));
                    }
                }
                Err(e) => {
                    res.insert("success".to_string(), Value::Bool(false));
                    res.insert("error".to_string(), Value::String(e.to_string()));
                }
            }

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.exec",
        Arc::new(|engine, _ctx, node, scope| {
            let mut command = String::new();
            let mut target = "exec_result".to_string();

            if node.value.is_some() {
                command = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "cmd" || child.name == "command" {
                    command = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let output = std::process::Command::new("bash")
                .args(&["-c", &command])
                .output();

            let mut res = HashMap::new();
            match output {
                Ok(out) => {
                    let exit_code = out.status.code().unwrap_or(-1);
                    res.insert("stdout".to_string(), Value::String(String::from_utf8_lossy(&out.stdout).to_string()));
                    res.insert("stderr".to_string(), Value::String(String::from_utf8_lossy(&out.stderr).to_string()));
                    res.insert("exit_code".to_string(), Value::Int(exit_code as i64));
                    res.insert("success".to_string(), Value::Bool(exit_code == 0));
                }
                Err(e) => {
                    res.insert("stdout".to_string(), Value::String(String::new()));
                    res.insert("stderr".to_string(), Value::String(e.to_string()));
                    res.insert("exit_code".to_string(), Value::Int(-1));
                    res.insert("success".to_string(), Value::Bool(false));
                }
            }

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ==========================================
    // PROCESS MANAGER SLOTS
    // ==========================================
    fn proc_info_to_value(info: &crate::procman::ProcessInfo) -> Value {
        let mut map = HashMap::new();
        map.insert("id".to_string(), Value::String(info.id.clone()));
        map.insert("name".to_string(), Value::String(info.name.clone()));
        map.insert("command".to_string(), Value::String(info.command.clone()));
        map.insert("cwd".to_string(), Value::String(info.cwd.clone()));
        
        let mut env_map = HashMap::new();
        for (k, v) in &info.env {
            env_map.insert(k.clone(), Value::String(v.clone()));
        }
        map.insert("env".to_string(), Value::Map(env_map));
        map.insert("auto_restart".to_string(), Value::Bool(info.auto_restart));
        map.insert("status".to_string(), Value::String(info.status.clone()));
        map.insert("pid".to_string(), match info.pid {
            Some(p) => Value::Int(p as i64),
            None => Value::Nil,
        });
        map.insert("exit_code".to_string(), match info.exit_code {
            Some(e) => Value::Int(e as i64),
            None => Value::Nil,
        });
        Value::Map(map)
    }

    engine.register(
        "proc.list",
        Arc::new(|_engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.list: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.list".to_string()),
                }
            })?;

            let mut target = "processes".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let list_fut = pm.list_processes();
            let list = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(list_fut)
            });

            let val_list = Value::List(list.iter().map(proc_info_to_value).collect());
            scope.set(&target, val_list);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.add",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.add: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.add".to_string()),
                }
            })?;

            let mut name = String::new();
            let mut command = String::new();
            let mut cwd = ".".to_string();
            let mut env = HashMap::new();
            let mut auto_restart = true;
            let mut target = "id".to_string();

            if node.value.is_some() {
                name = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "name" {
                    name = val.to_string_coerce();
                } else if child.name == "command" || child.name == "cmd" {
                    command = val.to_string_coerce();
                } else if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "auto_restart" {
                    auto_restart = val.to_bool();
                } else if child.name == "env" {
                    if let Value::Map(m) = val {
                        for (k, v) in m {
                            env.insert(k, v.to_string_coerce());
                        }
                    } else {
                        for env_child in &child.children {
                            let env_val = engine.resolve_shorthand_value(env_child, scope);
                            env.insert(env_child.name.clone(), env_val.to_string_coerce());
                        }
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let add_fut = pm.add_process(name, command, cwd, env, auto_restart);
            let id = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(add_fut)
            }).map_err(|e| Diagnostic {
                r#type: "error".to_string(),
                message: format!("proc.add failed: {}", e),
                filename: node.filename.clone(),
                line: node.line,
                col: node.col,
                slot: Some("proc.add".to_string()),
            })?;

            scope.set(&target, Value::String(id));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.update",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.update: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.update".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut name = String::new();
            let mut command = String::new();
            let mut cwd = ".".to_string();
            let mut env = HashMap::new();
            let mut auto_restart = true;
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
                } else if child.name == "command" || child.name == "cmd" {
                    command = val.to_string_coerce();
                } else if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "auto_restart" {
                    auto_restart = val.to_bool();
                } else if child.name == "env" {
                    if let Value::Map(m) = val {
                        for (k, v) in m {
                            env.insert(k, v.to_string_coerce());
                        }
                    } else {
                        for env_child in &child.children {
                            let env_val = engine.resolve_shorthand_value(env_child, scope);
                            env.insert(env_child.name.clone(), env_val.to_string_coerce());
                        }
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let update_fut = pm.update_process(&id, name, command, cwd, env, auto_restart);
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

    engine.register(
        "proc.start",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.start: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.start".to_string()),
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

            let start_fut = pm.start_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(start_fut)
            });

            println!("proc.start: id='{}', result={:?}", id, res);

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

    engine.register(
        "proc.stop",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.stop: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.stop".to_string()),
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

            let stop_fut = pm.stop_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(stop_fut)
            });

            println!("PROC.STOP: id='{}', target='{}', res={:?}", id, target, res);

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

    engine.register(
        "proc.restart",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.restart: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.restart".to_string()),
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

            let restart_fut = pm.restart_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(restart_fut)
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

    engine.register(
        "proc.delete",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.delete: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.delete".to_string()),
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

            let delete_fut = pm.remove_process(&id);
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

    engine.register(
        "proc.logs",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.logs: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.logs".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut lines = 100;
            let mut target = "logs".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "lines" {
                    lines = val.to_int() as usize;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let logs_fut = pm.get_logs(&id, lines);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(logs_fut)
            });

            match res {
                Ok(l) => {
                    scope.set(&target, Value::List(l.into_iter().map(Value::String).collect()));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::List(Vec::new()));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "if",
        Arc::new(|engine, ctx, node, scope| {
            let cond_val = if let Some(ref val_str) = node.value {
                evaluate_condition(engine, val_str, scope)
            } else {
                false
            };

            let mut then_node = None;
            let mut else_node = None;

            for child in &node.children {
                if child.name == "then" {
                    then_node = Some(child);
                } else if child.name == "else" {
                    else_node = Some(child);
                }
            }

            if cond_val {
                if let Some(then_n) = then_node {
                    for child in &then_n.children {
                        engine.execute(ctx, child, scope)?;
                    }
                }
            } else {
                if let Some(else_n) = else_node {
                    for child in &else_n.children {
                        engine.execute(ctx, child, scope)?;
                    }
                }
            }

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

fn evaluate_condition(engine: &Engine, expr: &str, scope: &Arc<Scope>) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return false;
    }

    if expr.contains("||") {
        for part in expr.split("||") {
            if evaluate_condition(engine, part, scope) {
                return true;
            }
        }
        return false;
    }

    if expr.contains("&&") {
        for part in expr.split("&&") {
            if !evaluate_condition(engine, part, scope) {
                return false;
            }
        }
        return true;
    }

    let ops = ["==", "!=", ">=", "<=", ">", "<"];
    for op in &ops {
        if expr.contains(op) {
            let parts: Vec<&str> = expr.splitn(2, op).collect();
            if parts.len() == 2 {
                let left_str = parts[0].trim();
                let right_str = parts[1].trim();

                let left_val = resolve_expression_value(engine, left_str, scope);
                let right_val = resolve_expression_value(engine, right_str, scope);

                return match *op {
                    "==" => left_val.to_string_coerce() == right_val.to_string_coerce(),
                    "!=" => left_val.to_string_coerce() != right_val.to_string_coerce(),
                    ">" => left_val.to_float() > right_val.to_float(),
                    "<" => left_val.to_float() < right_val.to_float(),
                    ">=" => left_val.to_float() >= right_val.to_float(),
                    "<=" => left_val.to_float() <= right_val.to_float(),
                    _ => false,
                };
            }
        }
    }

    let resolved = resolve_expression_value(engine, expr, scope);
    resolved.to_bool()
}

fn resolve_expression_value(_engine: &Engine, s: &str, scope: &Arc<Scope>) -> Value {
    let s = s.trim();
    if s.starts_with('$') {
        let key = &s[1..];
        return scope.get(key).unwrap_or(Value::Nil);
    }
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len()-1].to_string());
    }
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }
    if s == "null" || s == "nil" {
        return Value::Nil;
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    Value::String(s.to_string())
}

