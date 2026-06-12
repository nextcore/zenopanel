use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::{resolve_node_value, HttpResponseBuilder, send_json_response, value_to_serde_json};
use std::sync::Arc;
use std::collections::HashMap;

pub fn register(engine: &mut Engine) {
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
        "http.download_log",
        Arc::new(|engine, ctx, node, scope| {
            let mut proc_id = String::new();
            if node.value.is_some() {
                proc_id = resolve_node_value(engine, node, scope).to_string_coerce();
            }
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    proc_id = val.to_string_coerce();
                }
            }

            let log_path = format!("./logs/{}.log", proc_id);
            let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.download_log: not in HTTP context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.download_log".to_string()),
                }
            })?;

            if !std::path::Path::new(&log_path).exists() {
                *response_builder.status.lock().unwrap() = 404;
                response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), "application/json".to_string());
                *response_builder.body.lock().unwrap() = Some(r#"{"message":"Log file not found"}"#.to_string().into_bytes());
                return Ok(());
            }

            let content = std::fs::read_to_string(&log_path).unwrap_or_default();
            *response_builder.status.lock().unwrap() = 200;
            response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), "application/octet-stream".to_string());
            response_builder.headers.lock().unwrap().insert("Content-Disposition".to_string(), format!("attachment; filename=\"{}.log\"", proc_id));
            *response_builder.body.lock().unwrap() = Some(content.into_bytes());
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
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

    // --- http.request_path ---
    engine.register(
        "http.request_path",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "request_path".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default()
                        .trim_start_matches('$').to_string();
                }
            }
            let path = ctx.get::<String>("request_path")
                .map(|p| (*p).clone())
                .unwrap_or_default();
            scope.set(&target, Value::String(path));
            Ok(())
        }),
        SlotMeta { description: "Get current request path".to_string(), example: "http.request_path: { as: $path }".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "string".to_string() },
    );

    // --- http.request_method ---
    engine.register(
        "http.request_method",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "request_method".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default()
                        .trim_start_matches('$').to_string();
                }
            }
            let method = ctx.get::<String>("request_method")
                .map(|m| (*m).clone())
                .unwrap_or_else(|| "GET".to_string());
            scope.set(&target, Value::String(method));
            Ok(())
        }),
        SlotMeta { description: "Get current request method".to_string(), example: "http.request_method: { as: $method }".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "string".to_string() },
    );

    // --- http.redirect ---
    engine.register(
        "http.redirect",
        Arc::new(|engine, ctx, node, scope| {
            let mut location = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut status = 303u16;
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "to" || child.name == "location" {
                    location = val.to_string_coerce();
                } else if child.name == "status" {
                    status = val.to_int() as u16;
                }
            }
            let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                Diagnostic { r#type: "error".to_string(), message: "http.redirect: not in HTTP context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("http.redirect".to_string()) }
            })?;
            *response_builder.status.lock().unwrap() = status;
            response_builder.headers.lock().unwrap().insert("Location".to_string(), location);
            *response_builder.body.lock().unwrap() = Some(vec![]);
            Err(zenocore::Diagnostic { r#type: "halt".to_string(), message: "HALT".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("http.redirect".to_string()) })
        }),
        SlotMeta { description: "Redirect to another URL".to_string(), example: "http.redirect: '/'".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "nil".to_string() },
    );

    // --- http.csrf_token ---
    engine.register(
        "http.csrf_token",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "csrf_token".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default()
                        .trim_start_matches('$').to_string();
                }
            }
            let token = ctx.get::<String>("csrf_token")
                .map(|t| (*t).clone())
                .unwrap_or_default();
            scope.set(&target, Value::String(token));
            Ok(())
        }),
        SlotMeta { description: "Get current CSRF token".to_string(), example: "http.csrf_token: { as: $csrf }".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "string".to_string() },
    );

    // --- http.client_ip ---
    engine.register(
        "http.client_ip",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "client_ip".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default()
                        .trim_start_matches('$').to_string();
                }
            }
            let ip = if let Some(headers) = ctx.get::<axum::http::HeaderMap>("request_headers") {
                headers.get("X-Forwarded-For")
                    .or_else(|| headers.get("X-Real-IP"))
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "unknown".to_string()
            };
            scope.set(&target, Value::String(ip));
            Ok(())
        }),
        SlotMeta { description: "Get client IP address".to_string(), example: "http.client_ip: { as: $ip }".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "string".to_string() },
    );

    // --- http.save_uploads ---
    engine.register(
        "http.save_uploads",
        Arc::new(|_engine, ctx, node, scope| {
            let mut dest_dir = String::new();
            if node.value.is_some() {
                dest_dir = resolve_node_value(_engine, node, scope).to_string_coerce();
            }
            for child in &node.children {
                let val = _engine.resolve_shorthand_value(child, scope);
                if child.name == "path" || child.name == "dest" {
                    dest_dir = val.to_string_coerce();
                }
            }

            let headers = ctx.get::<axum::http::HeaderMap>("request_headers").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.save_uploads: request_headers not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.save_uploads".to_string()),
                }
            })?;
            let body_bytes_arc = ctx.get::<axum::body::Bytes>("request_body_bytes").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.save_uploads: request_body_bytes not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.save_uploads".to_string()),
                }
            })?;
            let body_bytes = (*body_bytes_arc).clone();

            let boundary = headers.get("content-type")
                .and_then(|h| h.to_str().ok())
                .and_then(|ct| multer::parse_boundary(ct).ok())
                .ok_or_else(|| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: "http.save_uploads: invalid or missing multipart boundary".to_string(),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("http.save_uploads".to_string()),
                    }
                })?;

            let stream = futures_util::stream::once(futures_util::future::ready(Ok::<_, std::io::Error>(body_bytes.clone())));
            let mut multipart = multer::Multipart::new(stream, boundary);

            let write_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut files = Vec::new();
                    let mut path_field = None;
                    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
                        let name = field.name().unwrap_or("").to_string();
                        if name == "path" {
                            if let Ok(text) = field.text().await {
                                path_field = Some(text);
                            }
                        } else if name == "file" || name == "files" || field.file_name().is_some() {
                            let filename = field.file_name().unwrap_or("uploaded_file").to_string();
                            let data = field.bytes().await.map_err(|e| e.to_string())?;
                            files.push((filename, data));
                        }
                    }
                    Ok::<_, String>((files, path_field))
                })
            });

            let (files, path_field) = match write_result {
                Ok(f) => f,
                Err(err_msg) => {
                    return Err(Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("http.save_uploads: failed to parse multipart: {}", err_msg),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("http.save_uploads".to_string()),
                    });
                }
            };

            if let Some(pf) = path_field {
                dest_dir = pf;
            }

            let dest_dir_path = std::path::Path::new(&dest_dir);
            if !dest_dir_path.exists() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("http.save_uploads: destination directory does not exist: {}", dest_dir),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.save_uploads".to_string()),
                });
            }

            let canonical_dir = dest_dir_path.canonicalize().map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("http.save_uploads: failed to canonicalize destination: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.save_uploads".to_string()),
                }
            })?;

            for (filename, bytes) in files {
                let safe_name = std::path::Path::new(&filename)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if safe_name.is_empty() || safe_name.starts_with('.') {
                    return Err(Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("http.save_uploads: invalid filename '{}'", filename),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("http.save_uploads".to_string()),
                    });
                }

                let file_dest = dest_dir_path.join(&safe_name);
                if let Some(parent) = file_dest.parent() {
                    if let Ok(canon_parent) = parent.canonicalize() {
                        if !canon_parent.starts_with(&canonical_dir) {
                            return Err(Diagnostic {
                                r#type: "error".to_string(),
                                message: "http.save_uploads: path traversal detected and rejected".to_string(),
                                filename: node.filename.clone(),
                                line: node.line,
                                col: node.col,
                                slot: Some("http.save_uploads".to_string()),
                            });
                        }
                    }
                }

                std::fs::write(&file_dest, bytes).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("http.save_uploads: failed to write file '{}': {}", safe_name, e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("http.save_uploads".to_string()),
                    }
                })?;
                println!("[FileManager] Uploaded file successfully to {:?}", file_dest);
            }

            let mut target = "success".to_string();
            let mut has_target = false;
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                    has_target = true;
                }
            }
            if has_target {
                scope.set(&target, Value::Bool(true));
            }

            Ok(())
        }),
        SlotMeta {
            description: "Process multipart file upload and save to dest directory".to_string(),
            example: "http.save_uploads: $dest_path { as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "bool".to_string(),
        },
    );
}

