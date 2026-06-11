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
}
