mod db;
mod slots;
pub mod procman;
pub mod proxyman;
pub mod sslman;
mod auth;
pub mod waf;
pub mod gateway;
pub mod backupman;


use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
    routing::any,
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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

/// Per-IP login attempt tracker for brute-force protection.
struct LoginAttempt {
    count: u32,
    first_at: Instant,
}

/// Simple in-memory rate limiter: max 5 failed attempts per 5 minutes per IP.
pub(crate) struct LoginLimiter {
    map: Mutex<HashMap<String, LoginAttempt>>,
}

impl LoginLimiter {
    const MAX_ATTEMPTS: u32 = 5;
    const WINDOW: Duration = Duration::from_secs(5 * 60);

    fn new() -> Self {
        Self { map: Mutex::new(HashMap::new()) }
    }

    /// Returns true if the IP is currently blocked.
    fn is_blocked(&self, ip: &str) -> bool {
        let map = self.map.lock().unwrap();
        if let Some(entry) = map.get(ip) {
            if entry.first_at.elapsed() < Self::WINDOW {
                return entry.count >= Self::MAX_ATTEMPTS;
            }
        }
        false
    }

    /// Record a failed attempt. Returns remaining attempts before lockout.
    fn record_failure(&self, ip: &str) -> u32 {
        let mut map = self.map.lock().unwrap();
        let entry = map.entry(ip.to_string()).or_insert_with(|| LoginAttempt {
            count: 0,
            first_at: Instant::now(),
        });
        // Reset window if expired
        if entry.first_at.elapsed() >= Self::WINDOW {
            entry.count = 0;
            entry.first_at = Instant::now();
        }
        entry.count += 1;
        Self::MAX_ATTEMPTS.saturating_sub(entry.count)
    }

    /// Clear attempts on successful login.
    fn clear(&self, ip: &str) {
        self.map.lock().unwrap().remove(ip);
    }
}

pub(crate) struct AppState {
    pub(crate) engine: Engine,
    pub(crate) router: MatchitRouter<MethodRouter>,
    pub(crate) parent_scope: Arc<Scope>,
    pub(crate) db_manager: DBManager,
    pub(crate) process_manager: Arc<crate::procman::ProcessManager>,
    pub(crate) proxy_manager: Arc<crate::proxyman::ProxyManager>,
    pub(crate) reqwest_client: reqwest::Client,
    pub(crate) csrf_enabled: bool,
    pub(crate) csrf_excepts: Vec<String>,
    pub(crate) jwt_secret: String,
    pub(crate) entrance_path: Mutex<String>,
    pub(crate) login_limiter: Arc<LoginLimiter>,
    pub(crate) rate_limiter: Arc<crate::waf::RateLimiter>,
    pub(crate) waf_enabled: std::sync::atomic::AtomicBool,
    pub(crate) traffic_stats: Arc<crate::waf::TrafficStatsManager>,
    pub(crate) backup_manager: Arc<crate::backupman::BackupManager>,
    pub(crate) mgmt_port: u16,
}

fn kill_process_on_port(port: u16) {
    let self_pid = std::process::id() as i32;

    // Method 1: lsof
    if let Ok(output) = std::process::Command::new("lsof")
        .args(&["-t", &format!("-i:{}", port)])
        .output()
    {
        let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !pid_str.is_empty() {
            for pid_line in pid_str.lines() {
                if let Ok(pid) = pid_line.parse::<i32>() {
                    if pid != self_pid {
                        println!("⚠️ Port {} is in use by process {}. Attempting to kill it...", port, pid);
                        let _ = std::process::Command::new("kill").args(&["-15", &pid.to_string()]).status();
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let _ = std::process::Command::new("kill").args(&["-9", &pid.to_string()]).status();
                    }
                }
            }
            return;
        }
    }

    // Method 2: fuser (fallback)
    if let Ok(output) = std::process::Command::new("fuser")
        .args(&[&format!("{}/tcp", port)])
        .output()
    {
        let merged_output = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for word in merged_output.split_whitespace() {
            if let Ok(pid) = word.parse::<i32>() {
                if pid != self_pid {
                    println!("⚠️ Port {} is in use by process {}. Attempting to kill it...", port, pid);
                    let _ = std::process::Command::new("kill").args(&["-15", &pid.to_string()]).status();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = std::process::Command::new("kill").args(&["-9", &pid.to_string()]).status();
                }
            }
        }
    }
}

fn kill_preexisting_processes() {
    let port = std::env::var("APP_PORT")
        .unwrap_or_else(|_| ":3000".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(3000);

    let tls_port = std::env::var("APP_TLS_PORT")
        .unwrap_or_else(|_| ":8443".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(8443);

    let mgmt_port = std::env::var("MGMT_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(3002);

    kill_process_on_port(port);
    kill_process_on_port(tls_port);
    kill_process_on_port(mgmt_port);
}

fn main() {
    // --- CLI Command Handling ---
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "key:generate" {
        handle_key_generate();
        return;
    }

    let _ = rustls::crypto::ring::default_provider().install_default();
    let _ = dotenvy::dotenv();

    kill_preexisting_processes();

    let rt = tokio::runtime::Runtime::new().unwrap();

    let (state, port, tls_port, cert_resolver) = rt.block_on(async {

    let csrf_enabled = std::env::var("CSRF_ENABLED").map(|v| v == "true").unwrap_or(true);
    let csrf_except_str = std::env::var("CSRF_EXCEPT").unwrap_or_else(|_| "/api,/health".to_string());
    let csrf_excepts: Vec<String> = csrf_except_str.split(',').map(|s| s.trim().to_string()).collect();

    let mut entrance_path = std::env::var("ENTRANCE_PATH").unwrap_or_else(|_| "/login".to_string());
    if !entrance_path.starts_with('/') {
        entrance_path = format!("/{}", entrance_path);
    }
    let admin_username = std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
    if !admin_password.starts_with("$2") && admin_password != "admin" {
        println!("⚠️ WARNING: ADMIN_PASSWORD is stored in plain text. For better security, consider hashing it using bcrypt.");
    }
    let placeholder = "zenopanel_local_development_jwt_secret_key_change_me_in_prod";
    let mut jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    if jwt_secret.is_empty() || jwt_secret == placeholder {
        jwt_secret = generate_secure_key();
        match write_jwt_to_env(&jwt_secret) {
            Ok(_)  => println!("🔑 JWT_SECRET otomatis di-generate dan disimpan ke .env"),
            Err(e) => eprintln!("⚠️ JWT_SECRET di-generate tapi gagal disimpan ke .env: {}", e),
        }
    }

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

    // Retrieve default pool to init ProcessManager
    let default_pool = match db_manager.get_pool("default").await {
        Some(crate::db::DbPool::Sqlite(pool)) => pool,
        _ => panic!("Default DB pool not initialized"),
    };

    // Create users table and seed initial admin if empty
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(&default_pool)
    .await
    .expect("Failed to create users table");

    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&default_pool)
        .await
        .unwrap_or((0,));

    if user_count.0 == 0 {
        let hashed_pw = if admin_password.starts_with("$2") {
            admin_password.clone()
        } else {
            bcrypt::hash(&admin_password, bcrypt::DEFAULT_COST).expect("Failed to hash admin password")
        };

        sqlx::query("INSERT INTO users (username, password_hash, role) VALUES (?, ?, ?)")
            .bind(&admin_username)
            .bind(&hashed_pw)
            .bind("admin")
            .execute(&default_pool)
            .await
            .expect("Failed to seed default admin user");
        println!("🚀 Seeded default admin user '{}' in the database.", admin_username);
    }

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )"
    )
    .execute(&default_pool)
    .await
    .expect("Failed to create settings table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS waf_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ip TEXT NOT NULL,
            reason TEXT NOT NULL,
            target TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(&default_pool)
    .await
    .expect("Failed to create waf_logs table");

    // Load custom entrance path if configured in DB
    if let Ok(Some((db_val,))) = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = 'entrance_path'")
        .fetch_optional(&default_pool)
        .await
    {
        entrance_path = db_val;
        if !entrance_path.starts_with('/') {
            entrance_path = format!("/{}", entrance_path);
        }
    }

    let process_manager = Arc::new(procman::ProcessManager::new(default_pool.clone()).await);
    if let Err(e) = process_manager.load_from_db().await {
        eprintln!("Failed to load processes from DB: {}", e);
    }

    let proxy_manager = Arc::new(proxyman::ProxyManager::new(default_pool.clone()).await);
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

    let main_zl_content = std::fs::read_to_string("zsrc/main.zl").expect("Failed to read zsrc/main.zl");
    let main_node = zenocore::parser::parse_string(&main_zl_content, "zsrc/main.zl").expect("Failed to parse zsrc/main.zl");

    let mut init_ctx = zenocore::Context::new();
    init_ctx.set("db_manager", db_manager.clone());
    init_ctx.set("process_manager", process_manager.clone());
    init_ctx.set("proxy_manager", proxy_manager.clone());

    if let Err(e) = engine.execute(&mut init_ctx, &main_node, &parent_scope) {
        panic!("Failed to execute zsrc/main.zl during startup: {}", e);
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

    let reqwest_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let login_limiter = Arc::new(LoginLimiter::new());

    let pool = &default_pool;
    
    let mut db_waf_enabled = std::env::var("WAF_ENABLED").map(|v| v == "true").unwrap_or(true);
    if let Ok(Some((db_val,))) = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = 'waf_enabled'")
        .fetch_optional(pool)
        .await
    {
        db_waf_enabled = db_val == "true";
    } else {
        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('waf_enabled', ?)")
            .bind(if db_waf_enabled { "true" } else { "false" })
            .execute(pool)
            .await;
    }

    let mut db_rate_limit_enabled = std::env::var("RATE_LIMIT_ENABLED").map(|v| v == "true").unwrap_or(true);
    if let Ok(Some((db_val,))) = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = 'rate_limit_enabled'")
        .fetch_optional(pool)
        .await
    {
        db_rate_limit_enabled = db_val == "true";
    } else {
        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_enabled', ?)")
            .bind(if db_rate_limit_enabled { "true" } else { "false" })
            .execute(pool)
            .await;
    }

    let mut db_rate_limit_max = std::env::var("RATE_LIMIT_MAX_REQUESTS").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(100);
    if let Ok(Some((db_val,))) = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = 'rate_limit_max'")
        .fetch_optional(pool)
        .await
    {
        if let Ok(val) = db_val.parse::<usize>() {
            db_rate_limit_max = val;
        }
    } else {
        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_max', ?)")
            .bind(db_rate_limit_max.to_string())
            .execute(pool)
            .await;
    }

    let mut db_rate_limit_window = std::env::var("RATE_LIMIT_WINDOW_SECS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(60);
    if let Ok(Some((db_val,))) = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = 'rate_limit_window'")
        .fetch_optional(pool)
        .await
    {
        if let Ok(val) = db_val.parse::<u64>() {
        db_rate_limit_window = val;
        }
    } else {
        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_window', ?)")
            .bind(db_rate_limit_window.to_string())
            .execute(pool)
            .await;
    }

    let rate_limiter = Arc::new(waf::RateLimiter::new(db_rate_limit_enabled, db_rate_limit_max, db_rate_limit_window));
    let traffic_stats = Arc::new(waf::TrafficStatsManager::new());

    let ts_clone = traffic_stats.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            ts_clone.tick();
        }
    });

    let mgmt_port = std::env::var("MGMT_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(3002);

    // Seed default settings for backup
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('backup_enabled', 'false')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('backup_interval_hours', '24')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('backup_retention', '7')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('backup_dest_dir', '/var/lib/zenopanel/backups')").execute(pool).await;
    let _ = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES ('backup_post_script', '/var/lib/zenopanel/backup-post.sh')").execute(pool).await;

    let backup_mgr = Arc::new(backupman::BackupManager::new(pool.clone()));
    backup_mgr.clone().start();

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
        jwt_secret,
        entrance_path: Mutex::new(entrance_path),
        login_limiter,
        rate_limiter,
        waf_enabled: std::sync::atomic::AtomicBool::new(db_waf_enabled),
        traffic_stats,
        backup_manager: backup_mgr,
        mgmt_port,
    });

    let static_service = ServeDir::new("public");

    let app = Router::new()
        .nest_service("/public", static_service)
        .fallback(any(wildcard_handler))
        .layer(axum::middleware::from_fn_with_state(state.clone(), waf::waf_middleware))
        .with_state(state.clone())
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(CorsLayer::permissive());

    let port = std::env::var("APP_PORT")
        .unwrap_or_else(|_| ":3000".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(3000);

    let tls_port = std::env::var("APP_TLS_PORT")
        .unwrap_or_else(|_| ":8443".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(8443);

    // Bind internal Axum server to localhost dynamic management port
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", mgmt_port)).await.unwrap();
    println!("Internal Axum server running on http://127.0.0.1:{}", mgmt_port);

    // Initialize shared certificate resolver
    let cert_resolver = Arc::new(sslman::ZenoCertResolver::new(proxy_manager.clone(), "./certs"));

    // Spawn SSL Auto-Renewal worker in the background
    let pm_renew_clone = proxy_manager.clone();
    let resolver_renew_clone = cert_resolver.clone();
    tokio::spawn(async move {
        sslman::start_auto_renewal_worker(pm_renew_clone, resolver_renew_clone).await;
    });

    // Spawn dynamic proxy listeners worker in the background
    let state_dynamic_clone = state.clone();
    let app_dynamic_clone = app.clone();
    tokio::spawn(async move {
        start_dynamic_proxy_listeners(state_dynamic_clone, app_dynamic_clone, port).await;
    });

    // Spawn the internal Axum server in the background
    let app_clone = app.clone();
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app_clone).await {
            eprintln!("Internal Axum server error: {}", e);
        }
    });

        (state, port, tls_port, cert_resolver)
    });

    // Start Pingora Reverse Proxy Gateway
    let mut pingora_server = pingora::server::Server::new(None).unwrap();
    pingora_server.bootstrap();

    let gateway = gateway::ZenoGateway::new(state.clone());
    let mut proxy_service = pingora::proxy::http_proxy_service(&pingora_server.configuration, gateway);

    // Configure HTTP listener
    proxy_service.add_tcp(&format!("0.0.0.0:{}", port));
    println!("Pingora Reverse Proxy Gateway listening on http://0.0.0.0:{}", port);

    // Configure HTTPS/TLS listener
    let default_cert_path = "./certs/default.crt";
    let default_key_path = "./certs/default.key";
    if !std::path::Path::new(default_cert_path).exists() || !std::path::Path::new(default_key_path).exists() {
        println!("[SSL] Creating default bootstrap self-signed certificate...");
        if let Ok(rcgen_key) = rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "zenopanel.local".to_string()]) {
            let _ = std::fs::create_dir_all("./certs");
            let _ = std::fs::write(default_cert_path, rcgen_key.cert.pem());
            let _ = std::fs::write(default_key_path, rcgen_key.key_pair.serialize_pem());
        }
    }

    if let Ok(mut tls_settings) = pingora::listeners::tls::TlsSettings::intermediate(default_cert_path, default_key_path) {
        tls_settings.enable_h2();

        let cert_resolver_clone = cert_resolver.clone();
        tls_settings.set_servername_callback(move |ssl, _alert| {
            if let Some(domain) = ssl.servername(openssl::ssl::NameType::HOST_NAME) {
                let cert_exists = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        cert_resolver_clone.get_or_create_cert(domain).await.is_some()
                    })
                });

                if cert_exists {
                    if let Some(ctx) = create_ssl_context_for_domain(domain) {
                        let _ = ssl.set_ssl_context(&ctx);
                    }
                }
            }
            Ok(())
        });

        proxy_service.add_tls_with_settings(&format!("0.0.0.0:{}", tls_port), None, tls_settings);
        println!("Pingora Reverse Proxy Gateway listening on https://0.0.0.0:{}", tls_port);
    } else {
        eprintln!("[SSL] Failed to initialize dynamic TLS settings for Pingora");
    }

    pingora_server.add_service(proxy_service);
    pingora_server.run_forever();
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

/// Generate a cryptographically random 64-character hex key for use as JWT_SECRET.
fn generate_secure_key() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Write (or replace) JWT_SECRET in the .env file.
/// Shared by both the startup auto-generate logic and the `key:generate` CLI command.
fn write_jwt_to_env(key: &str) -> std::io::Result<()> {
    let env_path = ".env";
    let existing = std::fs::read_to_string(env_path).unwrap_or_default();
    let mut found = false;
    let mut new_lines: Vec<String> = existing
        .lines()
        .map(|line| {
            if line.starts_with("JWT_SECRET=") {
                found = true;
                format!("JWT_SECRET={}", key)
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found {
        new_lines.push(format!("JWT_SECRET={}", key));
    }
    std::fs::write(env_path, new_lines.join("\n") + "\n")
}

/// Handle the `zeno key:generate` CLI command.
/// Generates a new JWT_SECRET and writes it to the .env file.
fn handle_key_generate() {
    let new_key = generate_secure_key();
    match write_jwt_to_env(&new_key) {
        Ok(_) => {
            println!("✅ JWT_SECRET berhasil di-generate dan disimpan ke .env");
            println!("   Key: {}", new_key);
        }
        Err(e) => {
            eprintln!("❌ Gagal menulis ke .env: {}", e);
            std::process::exit(1);
        }
    }
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
    req: axum::http::Request<axum::body::Body>,
) -> Response {
    let (parts, req_body) = req.into_parts();
    let method = parts.method;
    let uri = parts.uri;
    let headers: axum::http::HeaderMap = parts.headers;

    let path = uri.path();
    let method_str = method.as_str();

    let query_params: HashMap<String, String> = uri.query()
        .map(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_default();

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

    // Extract Host header and port
    let host_header = headers.get("Host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let (host, request_port_opt) = proxyman::parse_host_port(host_header);
    let request_port = request_port_opt.unwrap_or(80);

    // Check if request matches any proxy rule
    if let Some(rule) = state.proxy_manager.match_rule(&host, request_port, path).await {
        let mut check_service_unavailable = false;
        let mut app_status = "offline".to_string();
        let app_name = rule.name.clone();

        if let Some(ref proc_id) = rule.managed_process_id {
            if !proc_id.is_empty() {
                if let Some(status) = state.process_manager.get_process_status(proc_id).await {
                    app_status = status;
                    if app_status != "running" {
                        check_service_unavailable = true;
                    }
                }
            }
        }

        if check_service_unavailable {
            let html = render_error_page(&state.engine, &app_status, &app_name, &format!("The linked process ({}) has status: {}", rule.managed_process_id.as_deref().unwrap_or(""), app_status));
            return Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header("Content-Type", "text/html")
                .body(axum::body::Body::from(html))
                .unwrap();
        }

        if rule.rule_type == "static" {
            let target_path = std::path::Path::new(&rule.target);
            if !target_path.exists() || !target_path.is_dir() {
                let html = render_error_page(
                    &state.engine,
                    "failed",
                    &rule.name,
                    &format!("Target directory does not exist or is not a directory: {}", rule.target)
                );
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "text/html")
                    .body(axum::body::Body::from(html))
                    .unwrap();
            }

            let relative_path = if rule.strip_path {
                path.strip_prefix(&rule.path).unwrap_or(path)
            } else {
                path
            };

            let relative_path_slashed = if relative_path.starts_with('/') {
                relative_path.to_string()
            } else {
                format!("/{}", relative_path)
            };

            let uri_string = if let Some(query) = uri.query() {
                format!("{}?{}", relative_path_slashed, query)
            } else {
                relative_path_slashed
            };

            let mut req_for_serve = axum::http::Request::builder()
                .method(method.clone())
                .uri(uri_string);

            if let Some(headers_mut) = req_for_serve.headers_mut() {
                *headers_mut = headers.clone();
            }

            let req_for_serve = req_for_serve
                .body(axum::body::Body::empty())
                .unwrap();

            use tower::ServiceExt;
            let serve_dir = ServeDir::new(&rule.target)
                .precompressed_gzip()
                .precompressed_br()
                .precompressed_deflate()
                .precompressed_zstd()
                .fallback(tower_http::services::ServeFile::new(format!("{}/index.html", rule.target)));

            match serve_dir.oneshot(req_for_serve).await {
                Ok(response) => {
                    use axum::response::IntoResponse;
                    return response.into_response();
                }
                Err(e) => {
                    let html = render_error_page(
                        &state.engine,
                        "failed",
                        &rule.name,
                        &format!("Failed to serve static file: {}", e)
                    );
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/html")
                        .body(axum::body::Body::from(html))
                        .unwrap();
                }
            }
        }

        // Get the next target for load balancing
        let selected_target = state.proxy_manager.get_next_target(&rule.id, &rule.target).await;
        state.proxy_manager.increment_conn(&selected_target);

        let res = forward_request(&state.reqwest_client, &selected_target, &rule, method.clone(), uri.clone(), headers.clone(), req_body).await;
        state.proxy_manager.decrement_conn(&selected_target);

        match res {
            Ok(response) => return response,
            Err(e) => {
                eprintln!("Reverse proxy error for {} {}: {}", method, uri, e);
                let html = render_error_page(&state.engine, "failed", &rule.name, &format!("Connection failed: {}\n\nThe application may still be initializing or binding to its port.", e));
                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header("Content-Type", "text/html")
                    .body(axum::body::Body::from(html))
                    .unwrap();
            }
        }
    }

    // Read the body bytes in full (since this is an internal ZenoPanel request)
    let body_bytes = match axum::body::to_bytes(req_body, 100 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(axum::body::Body::from(format!("Failed to read request body: {}", e)))
                .unwrap();
        }
    };


    // 2. Check logout path (kept minimal in Rust as fallback for non-ZenoLang contexts)
    // Note: logout is now handled via ZenoLang in zsrc/routes/auth.zl
    // This block is intentionally removed — logout is a ZenoLang POST route.


    // 3. Secure ZenoPanel page requests and APIs (exclude the login path)
    let mut current_claims = None;
    if let Some(token) = auth::extract_token(&headers) {
        if let Ok(claims) = auth::verify_jwt(&token, &state.jwt_secret) {
            current_claims = Some(claims);
        }
    }

    // Role-based authorization and user management APIs
    if let Some(claims) = &current_claims {
        // 3. Database SQL Console API - ADMIN only
        if path.starts_with("/api/database/") || path.starts_with("/api/db/") {
            if claims.role != "admin" {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("Content-Type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "success": false,
                            "message": "Forbidden: Hanya Administrator yang diijinkan mengakses console Database"
                        })).unwrap()
                    ))
                    .unwrap();
            }
        }

        // 4. Interactive Terminal APIs - ADMIN only
        if path.starts_with("/api/terminal/") {
            if claims.role != "admin" {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("Content-Type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "success": false,
                            "message": "Forbidden: Hanya Administrator yang diijinkan menggunakan Terminal"
                        })).unwrap()
                    ))
                    .unwrap();
            }
        }

        // 5. Mutation blocks for VIEWERS
        if claims.role == "viewer" {
            let is_mutation = method == Method::POST || method == Method::PUT || method == Method::DELETE;
            if is_mutation && path != "/logout" {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("Content-Type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&serde_json::json!({
                            "success": false,
                            "message": "Forbidden: Role Viewer tidak diijinkan untuk melakukan modifikasi"
                        })).unwrap()
                    ))
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
    ctx.set("request_method", method_str.to_string());

    let response_builder = HttpResponseBuilder {
        status: std::sync::Mutex::new(200),
        headers: std::sync::Mutex::new(HashMap::new()),
        cookies: std::sync::Mutex::new(Vec::new()),
        body: std::sync::Mutex::new(None),
    };
    ctx.set("response_builder", response_builder);

    let html_buffer = zeno_blade::slots::HtmlBuffer(std::sync::Mutex::new(String::new()));
    ctx.set("httpWriter", html_buffer);

    ctx.set("db_manager", state.db_manager.clone());
    ctx.set("process_manager", state.process_manager.clone());
    ctx.set("proxy_manager", state.proxy_manager.clone());
    if let Some(claims) = &current_claims {
        ctx.set("user_claims", claims.clone());
    }
    ctx.set("app_state", state.clone());
    ctx.set("request_headers", headers.clone());
    ctx.set("request_path", path.to_string());
    ctx.set("request_body_bytes", body_bytes.clone());



    for child in &handler_node.children {
        if let Err(diag) = state.engine.execute(&mut ctx, child, &req_scope) {
            if diag.message == "HALT" {
                break;
            }
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

    let cookies_vec = response_builder_arc.cookies.lock().unwrap().clone();
    for cookie_val in cookies_vec {
        if let Ok(cookie_hdr) = axum::http::HeaderValue::from_str(&cookie_val) {
            response.headers_mut().append(axum::http::header::SET_COOKIE, cookie_hdr);
        }
    }

    if let Some(cookie_val) = new_cookie {
        if let Ok(cookie_hdr) = axum::http::HeaderValue::from_str(&cookie_val) {
            response.headers_mut().append(axum::http::header::SET_COOKIE, cookie_hdr);
        }
    }

    response
}

async fn forward_request(
    client: &reqwest::Client,
    selected_target: &str,
    rule: &crate::proxyman::ProxyRule,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    req_body: axum::body::Body,
) -> Result<Response, reqwest::Error> {
    let mut target_url = selected_target.trim_end_matches('/').to_string();
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

    // Wrap the axum body stream for reqwest
    let stream = req_body.into_data_stream();
    let req_body_stream = reqwest::Body::wrap_stream(stream);
    req_builder = req_builder.body(req_body_stream);

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

    let res_stream = res.bytes_stream();
    let response = builder.body(axum::body::Body::from_stream(res_stream)).unwrap();
    Ok(response)
}

pub(crate) fn render_error_page(engine: &zenocore::Engine, status: &str, app_name: &str, details: &str) -> String {
    let status_color = match status {
        "starting" => "#eab308", // Yellow
        "stopped" => "#6b7280", // Gray
        "failed" => "#ef4444", // Red
        _ => "#ef4444",
    };
    
    let status_label = match status {
        "starting" => "Starting",
        "stopped" => "Stopped",
        "failed" => "Failed / Crashed",
        _ => "Offline",
    };

    let status_desc = match status {
        "starting" => "The application is currently booting up. Please refresh in a few seconds.",
        "stopped" => "The application has been stopped by the system administrator.",
        "failed" => "The application process crashed or failed to start correctly. Check logs.",
        _ => "The proxy destination could not be reached. The service might be offline.",
    };

    let code = r#"
        view.blade: 'proxy_error' {
            status_color: $status_color
            status_label: $status_label
            app_name: $app_name
            status_desc: $status_desc
            details: $details
        }
    "#;

    match zenocore::parser::parse_string(code, "proxy_error_render") {
        Ok(node) => {
            let mut ctx = zenocore::Context::new();
            ctx.set("status_color", zenocore::Value::String(status_color.to_string()));
            ctx.set("status_label", zenocore::Value::String(status_label.to_string()));
            ctx.set("app_name", zenocore::Value::String(app_name.to_string()));
            ctx.set("status_desc", zenocore::Value::String(status_desc.to_string()));
            ctx.set("details", zenocore::Value::String(details.to_string()));

            let html_buffer = zeno_blade::slots::HtmlBuffer(std::sync::Mutex::new(String::new()));
            ctx.set("httpWriter", html_buffer);

            let scope = std::sync::Arc::new(zenocore::Scope::new(None));
            scope.set("status_color", zenocore::Value::String(status_color.to_string()));
            scope.set("status_label", zenocore::Value::String(status_label.to_string()));
            scope.set("app_name", zenocore::Value::String(app_name.to_string()));
            scope.set("status_desc", zenocore::Value::String(status_desc.to_string()));
            scope.set("details", zenocore::Value::String(details.to_string()));
            if let Err(e) = engine.execute(&mut ctx, &node, &scope) {
                eprintln!("Failed to execute proxy_error render node: {}", e);
                return format!("<html><body><h1>Service {} Unavailable</h1><p>{}</p><pre>{}</pre></body></html>", app_name, status_desc, details);
            }

            if let Some(html_buffer_arc) = ctx.get::<zeno_blade::slots::HtmlBuffer>("httpWriter") {
                let html = html_buffer_arc.0.lock().unwrap().clone();
                if !html.is_empty() {
                    return html;
                }
            }
            format!("<html><body><h1>Service {} Unavailable</h1><p>{}</p><pre>{}</pre></body></html>", app_name, status_desc, details)
        }
        Err(e) => {
            eprintln!("Failed to parse proxy_error template render snippet: {}", e);
            format!("<html><body><h1>Service {} Unavailable</h1><p>{}</p><pre>{}</pre></body></html>", app_name, status_desc, details)
        }
    }
}



pub(crate) async fn start_dynamic_proxy_listeners(
    state: Arc<AppState>,
    app: axum::Router,
    main_port: u16,
) {
    use std::collections::{HashMap, HashSet};
    use tokio::sync::oneshot;

    let mut active_listeners: HashMap<u16, oneshot::Sender<()>> = HashMap::new();
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let rules = state.proxy_manager.list_rules().await;
        let mut target_ports = HashSet::new();
        
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            for host_str in &[&rule.domain, &rule.alternative_domain] {
                let (_, port_opt) = proxyman::parse_host_port(host_str);
                if let Some(p) = port_opt {
                    // Do not bind to main port or standard SSL ports
                    if p != main_port && p != 8443 && p != 443 {
                        target_ports.insert(p);
                    }
                }
            }
        }
        
        // Stop listeners for ports no longer in use
        let mut ports_to_stop = Vec::new();
        for &port in active_listeners.keys() {
            if !target_ports.contains(&port) {
                ports_to_stop.push(port);
            }
        }
        for port in ports_to_stop {
            if let Some(tx) = active_listeners.remove(&port) {
                println!("[Proxy] Stopping dynamic listener on port {}", port);
                let _ = tx.send(());
            }
        }
        
        // Start listeners for new ports
        for &port in &target_ports {
            if !active_listeners.contains_key(&port) {
                match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
                    Ok(listener) => {
                        println!("[Proxy] Dynamically listening on http://0.0.0.0:{}", port);
                        let (tx, rx) = oneshot::channel::<()>();
                        let app_clone = app.clone();
                        
                        tokio::spawn(async move {
                            let serve_future = axum::serve(listener, app_clone)
                                .with_graceful_shutdown(async move {
                                    let _ = rx.await;
                                });
                            if let Err(e) = serve_future.await {
                                eprintln!("[Proxy] Error on dynamic port {} serve: {}", port, e);
                            }
                        });
                        
                        active_listeners.insert(port, tx);
                    }
                    Err(e) => {
                        eprintln!("[Proxy] Failed to bind to dynamic port {}: {}", port, e);
                    }
                }
            }
        }
    }
}

fn create_ssl_context_for_domain(domain: &str) -> Option<openssl::ssl::SslContext> {
    let cert_path = format!("./certs/{}.crt", domain);
    let key_path = format!("./certs/{}.key", domain);

    if !std::path::Path::new(&cert_path).exists() || !std::path::Path::new(&key_path).exists() {
        return None;
    }

    let cert_bytes = std::fs::read(&cert_path).ok()?;
    let key_bytes = std::fs::read(&key_path).ok()?;

    let mut builder = openssl::ssl::SslContext::builder(openssl::ssl::SslMethod::tls()).ok()?;

    // Parse certificates (handling PEM stack/chain or single DER/PEM)
    if let Ok(certs) = openssl::x509::X509::stack_from_pem(&cert_bytes) {
        if !certs.is_empty() {
            builder.set_certificate(&certs[0]).ok()?;
            for intermediate in certs.iter().skip(1) {
                builder.add_extra_chain_cert(intermediate.clone()).ok()?;
            }
        } else {
            let cert = openssl::x509::X509::from_der(&cert_bytes).ok()?;
            builder.set_certificate(&cert).ok()?;
        }
    } else {
        // Fallback to DER or single PEM
        if let Ok(cert) = openssl::x509::X509::from_pem(&cert_bytes) {
            builder.set_certificate(&cert).ok()?;
        } else if let Ok(cert) = openssl::x509::X509::from_der(&cert_bytes) {
            builder.set_certificate(&cert).ok()?;
        } else {
            return None;
        }
    }

    // Parse private key (DER)
    let pkey = openssl::pkey::PKey::private_key_from_der(&key_bytes).ok()?;
    builder.set_private_key(&pkey).ok()?;

    builder.check_private_key().ok()?;

    Some(builder.build())
}


