mod db;
mod slots;
pub mod procman;
pub mod proxyman;
pub mod sslman;

use axum::{
    body::Bytes,
    extract::{State, Query},
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
    routing::any,
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use matchit::Router as MatchitRouter;
use zenocore::{Engine, Node, Scope, Value};
use crate::db::DBManager;
use crate::slots::{register_custom_slots, HttpResponseBuilder};

struct MethodRouter {
    get: Option<Node>,
    post: Option<Node>,
}

struct AppState {
    engine: Engine,
    router: MatchitRouter<MethodRouter>,
    parent_scope: Arc<Scope>,
    db_manager: DBManager,
    process_manager: Arc<crate::procman::ProcessManager>,
    proxy_manager: Arc<crate::proxyman::ProxyManager>,
    reqwest_client: reqwest::Client,
    csrf_enabled: bool,
    csrf_excepts: Vec<String>,
}

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _ = dotenvy::dotenv();

    let db_manager = DBManager::new();
    
    let db_driver = std::env::var("DB_DRIVER").unwrap_or_else(|_| "sqlite".to_string());
    let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "./zeno.db".to_string());
    if db_driver == "sqlite" {
        if let Err(e) = db_manager.add_sqlite_connection("default", &db_name).await {
            eprintln!("Failed to connect to default database: {}", e);
        }
    }

    if let Ok(internal_db) = std::env::var("DB_INTERNAL_NAME") {
        if let Err(e) = db_manager.add_sqlite_connection("internal", &internal_db).await {
            eprintln!("Failed to connect to internal database: {}", e);
        }
    }

    let mut engine = zenoengine::new_engine();
    register_custom_slots(&mut engine);
    crate::proxyman::register_proxy_slots(&mut engine);

    // Retrieve default pool to init ProcessManager
    let default_pool = match db_manager.get_pool("default").await {
        Some(crate::db::DbPool::Sqlite(pool)) => pool,
        _ => panic!("Default DB pool not initialized"),
    };
    let process_manager = Arc::new(procman::ProcessManager::new(default_pool.clone()).await);
    if let Err(e) = process_manager.load_from_db().await {
        eprintln!("Failed to load processes from DB: {}", e);
    }

    let proxy_manager = Arc::new(proxyman::ProxyManager::new(default_pool).await);
    if let Err(e) = proxy_manager.load_from_db().await {
        eprintln!("Failed to load proxies from DB: {}", e);
    }

    #[derive(Clone)]
    struct RouteReg {
        method: String,
        path: String,
        node: Node,
    }
    let registered_routes = Arc::new(Mutex::new(Vec::new()));

    let reg_clone = registered_routes.clone();
    engine.register("http.get", Arc::new(move |_engine, _ctx, node, _scope| {
        let path = node.value.clone().unwrap_or_default().trim().to_string();
        let clean_path = if path.starts_with('\'') || path.starts_with('"') {
            path[1..path.len()-1].to_string()
        } else {
            path
        };
        reg_clone.lock().unwrap().push(RouteReg {
            method: "GET".to_string(),
            path: clean_path,
            node: node.clone(),
        });
        Ok(())
    }), zenocore::SlotMeta {
        description: "".to_string(),
        example: "".to_string(),
        inputs: HashMap::new(),
        required_blocks: Vec::new(),
        value_type: "".to_string(),
    });

    let reg_clone = registered_routes.clone();
    engine.register("http.post", Arc::new(move |_engine, _ctx, node, _scope| {
        let path = node.value.clone().unwrap_or_default().trim().to_string();
        let clean_path = if path.starts_with('\'') || path.starts_with('"') {
            path[1..path.len()-1].to_string()
        } else {
            path
        };
        reg_clone.lock().unwrap().push(RouteReg {
            method: "POST".to_string(),
            path: clean_path,
            node: node.clone(),
        });
        Ok(())
    }), zenocore::SlotMeta {
        description: "".to_string(),
        example: "".to_string(),
        inputs: HashMap::new(),
        required_blocks: Vec::new(),
        value_type: "".to_string(),
    });

    let parent_scope = Scope::new(None);
    parent_scope.set("DB_DRIVER", Value::String(db_driver));

    let main_zl_content = std::fs::read_to_string("src/main.zl").expect("Failed to read src/main.zl");
    let main_node = zenocore::parser::parse_string(&main_zl_content, "src/main.zl").expect("Failed to parse src/main.zl");

    let mut init_ctx = zenocore::Context::new();
    init_ctx.set("db_manager", db_manager.clone());
    init_ctx.set("process_manager", process_manager.clone());
    init_ctx.set("proxy_manager", proxy_manager.clone());

    if let Err(e) = engine.execute(&mut init_ctx, &main_node, &parent_scope) {
        panic!("Failed to execute src/main.zl during startup: {}", e);
    }

    let mut matchit_routes: HashMap<String, MethodRouter> = HashMap::new();
    let routes_list = registered_routes.lock().unwrap();
    for route in routes_list.iter() {
        let matchit_path = convert_path_to_matchit(&route.path);
        let entry = matchit_routes.entry(matchit_path).or_insert_with(|| MethodRouter {
            get: None,
            post: None,
        });
        if route.method == "GET" {
            entry.get = Some(route.node.clone());
        } else if route.method == "POST" {
            entry.post = Some(route.node.clone());
        }
    }

    let mut router = MatchitRouter::new();
    for (path, method_router) in matchit_routes {
        if let Err(e) = router.insert(&path, method_router) {
            eprintln!("Failed to insert route '{}' into router: {}", path, e);
        }
    }

    let csrf_enabled = std::env::var("CSRF_ENABLED").map(|v| v == "true").unwrap_or(true);
    let csrf_except_str = std::env::var("CSRF_EXCEPT").unwrap_or_else(|_| "/api,/health".to_string());
    let csrf_excepts: Vec<String> = csrf_except_str.split(',').map(|s| s.trim().to_string()).collect();

    let reqwest_client = reqwest::Client::new();
    let state = Arc::new(AppState {
        engine,
        router,
        parent_scope,
        db_manager,
        process_manager: process_manager.clone(),
        proxy_manager: proxy_manager.clone(),
        reqwest_client,
        csrf_enabled,
        csrf_excepts,
    });

    let static_service = ServeDir::new("public");

    let app = Router::new()
        .nest_service("/public", static_service)
        .fallback(any(wildcard_handler))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = std::env::var("APP_PORT")
        .unwrap_or_else(|_| ":3000".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(3000);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    println!("Server running on http://localhost:{}", port);

    // Initialize shared certificate resolver
    let cert_resolver = Arc::new(sslman::ZenoCertResolver::new(proxy_manager.clone(), "./certs"));

    // Spawn HTTPS/TLS server in the background
    let cert_resolver_server_clone = cert_resolver.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        sslman::run_tls_server(cert_resolver_server_clone, app_clone).await;
    });

    // Spawn SSL Auto-Renewal worker in the background
    let pm_renew_clone = proxy_manager.clone();
    let resolver_renew_clone = cert_resolver.clone();
    tokio::spawn(async move {
        sslman::start_auto_renewal_worker(pm_renew_clone, resolver_renew_clone).await;
    });

    axum::serve(listener, app).await.unwrap();
}

fn convert_path_to_matchit(path: &str) -> String {
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            result.push(':');
            while let Some(&next_c) = chars.peek() {
                if next_c == '}' {
                    chars.next();
                    break;
                }
                result.push(chars.next().unwrap());
            }
        } else if c == '*' {
            result.push_str("*path");
        } else {
            result.push(c);
        }
    }
    result
}

fn generate_random_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..chars.len());
            chars[idx] as char
        })
        .collect()
}

fn get_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(axum::http::header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .map(|pair| pair.trim())
                .find(|pair| pair.starts_with(name))
                .and_then(|pair| pair.split('=').nth(1))
                .map(|val| val.to_string())
        })
}

fn serde_json_to_value(val: serde_json::Value) -> Value {
    match val {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Nil
            }
        }
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(a) => Value::List(a.into_iter().map(serde_json_to_value).collect()),
        serde_json::Value::Object(o) => {
            let mut map = HashMap::new();
            for (k, v) in o {
                map.insert(k, serde_json_to_value(v));
            }
            Value::Map(map)
        }
    }
}

async fn wildcard_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    Query(query_params): Query<HashMap<String, String>>,
    body_bytes: Bytes,
) -> Response {
    let path = uri.path();
    let method_str = method.as_str();

    // Intercept ACME HTTP-01 challenges
    if path.starts_with("/.well-known/acme-challenge/") {
        let token = path.trim_start_matches("/.well-known/acme-challenge/");
        let challenges = sslman::ACME_CHALLENGES.lock().unwrap();
        if let Some(key_auth) = challenges.get(token) {
            println!("[SSL] Serving ACME challenge response for token: {}", token);
            return Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from(key_auth.clone()))
                .unwrap();
        }
    }

    // Extract Host header
    let host = headers.get("Host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();

    // Check if request matches any proxy rule
    if let Some(rule) = state.proxy_manager.match_rule(&host, path).await {
        match forward_request(&state.reqwest_client, &rule, method.clone(), uri.clone(), headers.clone(), body_bytes.clone()).await {
            Ok(response) => return response,
            Err(e) => {
                eprintln!("Reverse proxy error for {} {}: {}", method, uri, e);
                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header("Content-Type", "text/plain")
                    .body(axum::body::Body::from(format!("Bad Gateway: {}", e)))
                    .unwrap();
            }
        }
    }

    let mut new_cookie = None;
    let mut csrf_token = String::new();
    if state.csrf_enabled {
        csrf_token = match get_cookie_value(&headers, "_csrf") {
            Some(token) => token,
            None => {
                let token = generate_random_token();
                new_cookie = Some(format!("_csrf={}; Path=/; SameSite=Lax", token));
                token
            }
        };

        if method == Method::POST || method == Method::PUT || method == Method::DELETE {
            let is_excepted = state.csrf_excepts.iter().any(|except| {
                if except.ends_with('*') {
                    path.starts_with(&except[..except.len() - 1])
                } else {
                    path == except || path.starts_with(&format!("{}/", except))
                }
            });

            if !is_excepted {
                let client_token = headers.get("X-CSRF-Token")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("");
                if client_token.is_empty() || client_token != csrf_token {
                    return Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("Content-Type", "text/plain")
                        .body(axum::body::Body::from("CSRF Token Mismatch"))
                        .unwrap();
                }
            }
        }
    }

    let matched = match state.router.at(path) {
        Ok(m) => m,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from("Halaman tidak ditemukan."))
                .unwrap();
        }
    };

    let handler_node = match method_str {
        "GET" => matched.value.get.as_ref(),
        "POST" => matched.value.post.as_ref(),
        _ => None,
    };

    let handler_node = match handler_node {
        Some(node) => node,
        None => {
            return Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from("Method Not Allowed"))
                .unwrap();
        }
    };

    let mut ctx = zenocore::Context::new();
    let req_scope = Scope::new(Some(state.parent_scope.clone()));

    let mut params_map = HashMap::new();
    for (k, v) in matched.params.iter() {
        req_scope.set(k, Value::String(v.to_string()));
        params_map.insert(k.to_string(), Value::String(v.to_string()));
    }
    req_scope.set("params", Value::Map(params_map));

    ctx.set("query_params", query_params);

    if method == Method::POST {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            let zeno_val = serde_json_to_value(json_val);
            ctx.set("json_body", zeno_val);
        }
    }

    if state.csrf_enabled {
        ctx.set("csrf_token", csrf_token);
    }

    let response_builder = HttpResponseBuilder {
        status: std::sync::Mutex::new(200),
        headers: std::sync::Mutex::new(HashMap::new()),
        body: std::sync::Mutex::new(None),
    };
    ctx.set("response_builder", response_builder);

    let html_buffer = zeno_blade::slots::HtmlBuffer(std::sync::Mutex::new(String::new()));
    ctx.set("httpWriter", html_buffer);

    ctx.set("db_manager", state.db_manager.clone());
    ctx.set("process_manager", state.process_manager.clone());
    ctx.set("proxy_manager", state.proxy_manager.clone());



    for child in &handler_node.children {
        if let Err(diag) = state.engine.execute(&mut ctx, child, &req_scope) {
            eprintln!("Execution error: {}", diag);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from(format!("Internal Server Error: {}", diag)))
                .unwrap();
        }
    }

    let response_builder_arc = ctx.get::<HttpResponseBuilder>("response_builder").unwrap();
    let status_code = *response_builder_arc.status.lock().unwrap();
    let headers_map = response_builder_arc.headers.lock().unwrap().clone();
    let body_opt = response_builder_arc.body.lock().unwrap().take();

    let html_buffer_arc = ctx.get::<zeno_blade::slots::HtmlBuffer>("httpWriter").unwrap();
    let mut response = if let Some(body_bytes) = body_opt {
        Response::new(axum::body::Body::from(body_bytes))
    } else {
        let html = html_buffer_arc.0.lock().unwrap().clone();
        if !html.is_empty() {
            let mut res = Response::new(axum::body::Body::from(html));
            res.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
            );
            res
        } else {
            Response::new(axum::body::Body::empty())
        }
    };

    *response.status_mut() = StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK);
    for (k, v) in headers_map {
        if let (Ok(h_name), Ok(h_val)) = (axum::http::HeaderName::from_bytes(k.as_bytes()), axum::http::HeaderValue::from_str(&v)) {
            response.headers_mut().insert(h_name, h_val);
        }
    }

    if let Some(cookie_val) = new_cookie {
        if let Ok(cookie_hdr) = axum::http::HeaderValue::from_str(&cookie_val) {
            response.headers_mut().insert(axum::http::header::SET_COOKIE, cookie_hdr);
        }
    }

    response
}

async fn forward_request(
    client: &reqwest::Client,
    rule: &crate::proxyman::ProxyRule,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body_bytes: Bytes,
) -> Result<Response, reqwest::Error> {
    let mut target_url = rule.target.trim_end_matches('/').to_string();
    let path_to_forward = if rule.strip_path {
        let prefix = rule.path.trim_end_matches('/');
        if !prefix.is_empty() {
            uri.path().strip_prefix(prefix).unwrap_or(uri.path())
        } else {
            uri.path()
        }
    } else {
        uri.path()
    };

    if !target_url.ends_with('/') && !path_to_forward.starts_with('/') {
        target_url.push('/');
    } else if target_url.ends_with('/') && path_to_forward.starts_with('/') {
        target_url.pop();
    }
    target_url.push_str(path_to_forward);

    if let Some(query) = uri.query() {
        target_url.push('?');
        target_url.push_str(query);
    }

    let mut req_builder = client.request(method.clone(), &target_url);

    let mut req_headers = reqwest::header::HeaderMap::new();
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if name_str == "host"
            || name_str == "content-length"
            || name_str == "connection"
            || name_str == "keep-alive"
            || name_str == "proxy-connection"
            || name_str == "transfer-encoding"
            || name_str == "upgrade"
        {
            continue;
        }
        req_headers.insert(name.clone(), value.clone());
    }

    if let Some(host_val) = headers.get("Host").and_then(|h| h.to_str().ok()) {
        if let Ok(hdr) = reqwest::header::HeaderValue::from_str(host_val) {
            req_headers.insert("X-Forwarded-Host", hdr);
        }
    }

    if let Ok(hdr) = reqwest::header::HeaderValue::from_str("http") {
        req_headers.insert("X-Forwarded-Proto", hdr);
    }

    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes);
    }

    req_builder = req_builder.headers(req_headers);

    let res = req_builder.send().await?;

    let mut builder = Response::builder().status(res.status().as_u16());
    let builder_headers = builder.headers_mut().unwrap();
    for (name, value) in res.headers().iter() {
        let name_str = name.as_str().to_lowercase();
        if name_str == "connection" || name_str == "transfer-encoding" {
            continue;
        }
        builder_headers.insert(name.clone(), value.clone());
    }

    let bytes = res.bytes().await?;
    let response = builder.body(axum::body::Body::from(bytes)).unwrap();
    Ok(response)
}

