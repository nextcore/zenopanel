use zenocore::{Engine, SlotMeta, Value};
use crate::auth::Claims;
use crate::AppState;
use super::{resolve_node_value, HttpResponseBuilder};
use std::collections::HashMap;
use std::sync::Arc;

pub fn register(engine: &mut Engine) {
    engine.register(
        "auth.is_admin",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "is_admin".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let is_admin = if let Some(claims) = ctx.get::<Claims>("user_claims") {
                claims.role == "admin"
            } else {
                // Fallback: read from scope claims set by auth.guard
                if let Some(claims_val) = scope.get("claims") {
                    if let Value::Map(ref m) = claims_val {
                        let role = m.get("role").map(|r| r.to_string_coerce()).unwrap_or_default();
                        role == "admin"
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            scope.set(&target, Value::Bool(is_admin));
            Ok(())
        }),
        SlotMeta {
            description: "Check if current user is an admin".to_string(),
            example: "auth.is_admin { as: $is_admin }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "bool".to_string(),
        },
    );

    engine.register(
        "auth.has_role",
        Arc::new(|engine, ctx, node, scope| {
            let required_role = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut target = "has_role".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let claims_opt = ctx.get::<Claims>("user_claims");
            let has_role = if let Some(claims) = claims_opt {
                match required_role.as_str() {
                    "admin" => claims.role == "admin",
                    "editor" => claims.role == "admin" || claims.role == "editor",
                    "viewer" => claims.role == "admin" || claims.role == "editor" || claims.role == "viewer",
                    _ => false,
                }
            } else {
                // Fallback: read from scope claims set by auth.guard
                if let Some(claims_val) = scope.get("claims") {
                    if let Value::Map(ref m) = claims_val {
                        let role = m.get("role").map(|r| r.to_string_coerce()).unwrap_or_default();
                        match required_role.as_str() {
                            "admin" => role == "admin",
                            "editor" => role == "admin" || role == "editor",
                            "viewer" => role == "admin" || role == "editor" || role == "viewer",
                            _ => false,
                        }
                    } else { false }
                } else { false }
            };

            scope.set(&target, Value::Bool(has_role));
            Ok(())
        }),
        SlotMeta {
            description: "Check if current user has the required role (with hierarchy)".to_string(),
            example: "auth.has_role: 'editor' { as: $has_role }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "bool".to_string(),
        },
    );

    engine.register(
        "system.update_entrance_path",
        Arc::new(|engine, ctx, node, scope| {
            let mut new_path = resolve_node_value(engine, node, scope).to_string_coerce();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    new_path = val.to_string_coerce();
                }
            }

            let mut success = false;
            let mut normalized = String::new();
            if let Some(state) = ctx.get::<Arc<AppState>>("app_state") {
                let mut new_path_trimmed = new_path.trim().to_string();
                if !new_path_trimmed.is_empty() {
                    if !new_path_trimmed.starts_with('/') {
                        new_path_trimmed = format!("/{}", new_path_trimmed);
                    }
                    normalized = new_path_trimmed.clone();
                    *state.entrance_path.lock().unwrap() = new_path_trimmed;
                    success = true;
                }
            }

            if success {
                scope.set("normalized_path", Value::String(normalized));
            }

            let mut target = "success".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }
            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Update the dynamic entrance path in-memory".to_string(),
            example: "system.update_entrance_path: $new_path { as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "bool".to_string(),
        },
    );

    engine.register(
        "auth.claims",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "claims".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let val = if let Some(claims) = ctx.get::<Claims>("user_claims") {
                let mut map = HashMap::new();
                map.insert("username".to_string(), Value::String(claims.sub.clone()));
                map.insert("role".to_string(), Value::String(claims.role.clone()));
                Value::Map(map)
            } else if let Some(claims_val) = scope.get("claims") {
                // Fallback: use scope claims set by auth.guard
                claims_val
            } else {
                Value::Nil
            };

            scope.set(&target, val);
            Ok(())
        }),
        SlotMeta {
            description: "Get current authenticated user claims".to_string(),
            example: "auth.claims { as: $claims }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "map".to_string(),
        },
    );

    engine.register(
        "crypto.bcrypt",
        Arc::new(|engine, _ctx, node, scope| {
            let mut password_plain = resolve_node_value(engine, node, scope).to_string_coerce();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "password" || child.name == "val" {
                    password_plain = val.to_string_coerce();
                }
            }

            let hashed = bcrypt::hash(&password_plain, bcrypt::DEFAULT_COST).unwrap_or_default();

            let mut target = "hashed".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }
            scope.set(&target, Value::String(hashed));
            Ok(())
        }),
        SlotMeta {
            description: "Hash a plain text password using bcrypt".to_string(),
            example: "crypto.bcrypt: $password { as: $hashed }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "string".to_string(),
        },
    );

    engine.register(
        "auth.guard",
        Arc::new(|_engine, ctx, node, scope| {
            let headers = ctx.get::<axum::http::HeaderMap>("request_headers").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "auth.guard: request_headers not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.guard".to_string()),
                }
            })?;
            let path = ctx.get::<String>("request_path").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "auth.guard: request_path not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.guard".to_string()),
                }
            })?;
            let state = ctx.get::<Arc<AppState>>("app_state").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "auth.guard: app_state not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.guard".to_string()),
                }
            })?;

            let mut authenticated = false;
            let mut current_claims = None;

            if let Some(token) = crate::auth::extract_token(&headers) {
                if let Ok(claims) = crate::auth::verify_jwt(&token, &state.jwt_secret) {
                    authenticated = true;
                    current_claims = Some(claims);
                }
            }

            if authenticated {
                if let Some(claims) = current_claims {
                    ctx.set("user_claims", claims.clone());
                    
                    let mut claims_map = HashMap::new();
                    claims_map.insert("username".to_string(), Value::String(claims.sub.clone()));
                    claims_map.insert("role".to_string(), Value::String(claims.role.clone()));
                    scope.set("claims", Value::Map(claims_map));
                }
                Ok(())
            } else {
                let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                    zenocore::Diagnostic {
                        r#type: "error".to_string(),
                        message: "auth.guard: HttpResponseBuilder not found".to_string(),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("auth.guard".to_string()),
                    }
                })?;

                if path.starts_with("/api/") {
                    *response_builder.status.lock().unwrap() = 401;
                    response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), "application/json".to_string());
                    let json_body = serde_json::json!({
                        "success": false,
                        "message": "Unauthorized: Silakan login terlebih dahulu"
                    });
                    *response_builder.body.lock().unwrap() = Some(serde_json::to_string(&json_body).unwrap().into_bytes());
                } else {
                    *response_builder.status.lock().unwrap() = 404;
                    response_builder.headers.lock().unwrap().insert("Content-Type".to_string(), "text/plain; charset=utf-8".to_string());
                    *response_builder.body.lock().unwrap() = Some("Halaman tidak ditemukan.".to_string().into_bytes());
                }

                Err(zenocore::Diagnostic {
                    r#type: "halt".to_string(),
                    message: "HALT".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.guard".to_string()),
                })
            }
        }),
        SlotMeta {
            description: "Authenticate request via JWT and halt on failure".to_string(),
            example: "auth.guard".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "nil".to_string(),
        },
    );

    engine.register(
        "http.set_cookie",
        Arc::new(|engine, ctx, node, scope| {
            let cookie_name = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut value = String::new();
            let mut path = "/".to_string();
            let mut httponly = false;
            let mut max_age: Option<i64> = None;
            let mut samesite = "Lax".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "value" {
                    value = val.to_string_coerce();
                } else if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "httponly" {
                    httponly = val.to_bool();
                } else if child.name == "max_age" {
                    max_age = Some(val.to_int());
                } else if child.name == "samesite" {
                    samesite = val.to_string_coerce();
                }
            }

            let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.set_cookie: HttpResponseBuilder not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.set_cookie".to_string()),
                }
            })?;

            let mut cookie_str = format!("{}={}; Path={}; SameSite={}", cookie_name, value, path, samesite);
            if httponly {
                cookie_str.push_str("; HttpOnly");
            }
            if let Some(age) = max_age {
                cookie_str.push_str(&format!("; Max-Age={}", age));
            }

            response_builder.cookies.lock().unwrap().push(cookie_str);
            Ok(())
        }),
        SlotMeta {
            description: "Set an HTTP cookie".to_string(),
            example: "http.set_cookie: 'cookie_name' { value: 'cookie_value', httponly: true }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "nil".to_string(),
        },
    );

    engine.register(
        "http.clear_cookie",
        Arc::new(|engine, ctx, node, scope| {
            let cookie_name = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut path = "/".to_string();
            let mut samesite = "Lax".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "samesite" {
                    samesite = val.to_string_coerce();
                }
            }

            let response_builder = ctx.get::<HttpResponseBuilder>("response_builder").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "http.clear_cookie: HttpResponseBuilder not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("http.clear_cookie".to_string()),
                }
            })?;

            let cookie_str = format!("{}={}; Path={}; Max-Age=0; SameSite={}", cookie_name, "", path, samesite);
            response_builder.cookies.lock().unwrap().push(cookie_str);
            Ok(())
        }),
        SlotMeta {
            description: "Clear an HTTP cookie".to_string(),
            example: "http.clear_cookie: 'cookie_name'".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "nil".to_string(),
        },
    );

    engine.register(
        "auth.generate_token",
        Arc::new(|engine, ctx, node, scope| {
            let username = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut role = "viewer".to_string();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "role" {
                    role = val.to_string_coerce();
                }
            }

            let state = ctx.get::<Arc<AppState>>("app_state").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "auth.generate_token: app_state not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.generate_token".to_string()),
                }
            })?;

            let token = crate::auth::generate_jwt(&username, &role, &state.jwt_secret).unwrap_or_default();

            let mut target = "token".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }
            scope.set(&target, Value::String(token));
            Ok(())
        }),
        SlotMeta {
            description: "Generate JWT token for user".to_string(),
            example: "auth.generate_token: $username { role: $role, as: $token }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "string".to_string(),
        },
    );

    engine.register(
        "auth.verify_password",
        Arc::new(|engine, ctx, node, scope| {
            let password_plain = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut username = String::new();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "username" {
                    username = val.to_string_coerce();
                }
            }

            let state = ctx.get::<Arc<AppState>>("app_state").ok_or_else(|| {
                zenocore::Diagnostic {
                    r#type: "error".to_string(),
                    message: "auth.verify_password: app_state not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("auth.verify_password".to_string()),
                }
            })?;

            let db_pool = match tokio::runtime::Handle::current().block_on(async {
                state.db_manager.get_pool("default").await
            }) {
                Some(crate::db::DbPool::Sqlite(pool)) => pool,
                _ => {
                    return Err(zenocore::Diagnostic {
                        r#type: "error".to_string(),
                        message: "auth.verify_password: SQLite pool not initialized".to_string(),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("auth.verify_password".to_string()),
                    });
                }
            };

            let user_row: Option<(String, String)> = tokio::runtime::Handle::current().block_on(async {
                sqlx::query_as("SELECT password_hash, role FROM users WHERE username = ?")
                    .bind(&username)
                    .fetch_optional(&db_pool)
                    .await
                    .unwrap_or(None)
            });

            let mut success = false;
            let mut role = String::new();

            if let Some((password_hash, user_role)) = user_row {
                if bcrypt::verify(&password_plain, &password_hash).unwrap_or(false) {
                    success = true;
                    role = user_role;
                }
            }

            let mut result_map = HashMap::new();
            result_map.insert("success".to_string(), Value::Bool(success));
            result_map.insert("role".to_string(), Value::String(role));

            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }
            scope.set(&target, Value::Map(result_map));
            Ok(())
        }),
        SlotMeta {
            description: "Verify username and password against users table".to_string(),
            example: "auth.verify_password: $password { username: $username, as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "map".to_string(),
        },
    );
}
