use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use once_cell::sync::Lazy;
use regex::Regex;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct RateLimiter {
    requests: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    enabled: AtomicBool,
    max_requests: AtomicUsize,
    window: Mutex<Duration>,
}

impl RateLimiter {
    pub fn new(enabled: bool, max_requests: usize, window_secs: u64) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            enabled: AtomicBool::new(enabled),
            max_requests: AtomicUsize::new(max_requests),
            window: Mutex::new(Duration::from_secs(window_secs)),
        }
    }

    pub fn check_limit(&self, ip: IpAddr) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return true;
        }
        let now = Instant::now();
        let mut reqs = self.requests.lock().unwrap();
        
        let ip_reqs = reqs.entry(ip).or_insert_with(Vec::new);
        let window = *self.window.lock().unwrap();
        let window_start = now.checked_sub(window).unwrap_or(now);
        ip_reqs.retain(|&t| t > window_start);

        if ip_reqs.len() >= self.max_requests.load(Ordering::Relaxed) {
            false
        } else {
            ip_reqs.push(now);
            true
        }
    }

    pub fn update(&self, enabled: bool, max_requests: usize, window_secs: u64) {
        self.enabled.store(enabled, Ordering::Relaxed);
        self.max_requests.store(max_requests, Ordering::Relaxed);
        *self.window.lock().unwrap() = Duration::from_secs(window_secs);
        self.requests.lock().unwrap().clear();
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn max_requests(&self) -> usize {
        self.max_requests.load(Ordering::Relaxed)
    }

    pub fn window_secs(&self) -> u64 {
        self.window.lock().unwrap().as_secs()
    }
}

use std::sync::atomic::AtomicU64;
use std::collections::VecDeque;

#[derive(Clone, serde::Serialize)]
pub struct TrafficMetric {
    pub timestamp: u64,
    pub requests: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub latency_ms: u64,
}

pub struct TrafficStatsManager {
    current_requests: AtomicUsize,
    current_bytes_sent: AtomicU64,
    current_bytes_received: AtomicU64,
    current_latency_sum: AtomicU64,
    current_latency_count: AtomicU64,
    history: Mutex<VecDeque<TrafficMetric>>,
}

impl TrafficStatsManager {
    pub fn new() -> Self {
        Self {
            current_requests: AtomicUsize::new(0),
            current_bytes_sent: AtomicU64::new(0),
            current_bytes_received: AtomicU64::new(0),
            current_latency_sum: AtomicU64::new(0),
            current_latency_count: AtomicU64::new(0),
            history: Mutex::new(VecDeque::new()),
        }
    }

    pub fn record(&self, bytes_received: u64, bytes_sent: u64, latency_ms: u64) {
        self.current_requests.fetch_add(1, Ordering::Relaxed);
        self.current_bytes_received.fetch_add(bytes_received, Ordering::Relaxed);
        self.current_bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
        self.current_latency_sum.fetch_add(latency_ms, Ordering::Relaxed);
        self.current_latency_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn tick(&self) {
        let reqs = self.current_requests.swap(0, Ordering::Relaxed);
        let rx = self.current_bytes_received.swap(0, Ordering::Relaxed);
        let tx = self.current_bytes_sent.swap(0, Ordering::Relaxed);
        let lat_sum = self.current_latency_sum.swap(0, Ordering::Relaxed);
        let lat_count = self.current_latency_count.swap(0, Ordering::Relaxed);
        let avg_latency = if lat_count > 0 { lat_sum / lat_count } else { 0 };

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let metric = TrafficMetric {
            timestamp: now_sec,
            requests: reqs,
            bytes_sent: tx,
            bytes_received: rx,
            latency_ms: avg_latency,
        };

        let mut hist = self.history.lock().unwrap();
        hist.push_back(metric);
        if hist.len() > 60 {
            hist.pop_front();
        }
    }

    pub fn get_history(&self) -> Vec<TrafficMetric> {
        let hist = self.history.lock().unwrap();
        hist.iter().cloned().collect()
    }
}

pub fn get_client_ip(headers: &HeaderMap, connect_info: Option<&ConnectInfo<SocketAddr>>) -> IpAddr {
    // 1. Check CF-Connecting-IP
    if let Some(cf_ip) = headers.get("CF-Connecting-IP") {
        if let Ok(ip_str) = cf_ip.to_str() {
            if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // 2. Check X-Forwarded-For
    if let Some(xff) = headers.get("X-Forwarded-For") {
        if let Ok(xff_str) = xff.to_str() {
            if let Some(first_ip) = xff_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // 3. Fallback to SocketAddr
    if let Some(ConnectInfo(addr)) = connect_info {
        addr.ip()
    } else {
        IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
    }
}

static SQLI_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(union\s+all\s+select|select\s+.*\s+from|insert\s+into|delete\s+from|drop\s+table|update\s+.*\s+set|or\s+\d+=\d+|--|#)").unwrap()
});

static XSS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(<script>|javascript:|onerror\s*=|onload\s*=|alert\(|document\.cookie|<img\s+src|expression\()").unwrap()
});

static PATH_TRAVERSAL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\.\./|\.\.\\|/etc/passwd|/etc/shadow|/win.ini|/boot.ini)").unwrap()
});

static RCE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(/bin/bash|/bin/sh|cmd\.exe|powershell|curl\s+http|wget\s+http)").unwrap()
});

pub fn is_malicious(input: &str) -> Option<&'static str> {
    // Decode percent encoding first if possible to catch evasion
    let decoded = urlencoding::decode(input).map(|cow| cow.into_owned()).unwrap_or_else(|_| input.to_string());
    
    if SQLI_REGEX.is_match(&decoded) {
        return Some("SQL Injection Pattern");
    }
    if XSS_REGEX.is_match(&decoded) {
        return Some("Cross-Site Scripting (XSS) Pattern");
    }
    if PATH_TRAVERSAL_REGEX.is_match(&decoded) {
        return Some("Path Traversal Pattern");
    }
    if RCE_REGEX.is_match(&decoded) {
        return Some("Remote Code Execution Pattern");
    }
    None
}

fn render_blocked_page(reason: &str, ip: IpAddr) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Request Blocked - ZenoPanel WAF</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{
            font-family: 'Inter', system-ui, -apple-system, sans-serif;
            background-color: #0d1117;
            color: #c9d1d9;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
        }}
        .card {{
            background-color: #161b22;
            border: 1px solid #30363d;
            border-radius: 12px;
            padding: 40px;
            max-width: 500px;
            width: 90%;
            box-shadow: 0 8px 24px rgba(0,0,0,0.5);
            text-align: center;
        }}
        h1 {{
            color: #f85149;
            margin-top: 0;
            font-size: 24px;
        }}
        p {{
            line-height: 1.6;
            font-size: 15px;
        }}
        .details {{
            background-color: #0d1117;
            border: 1px solid #30363d;
            padding: 15px;
            border-radius: 6px;
            text-align: left;
            font-family: monospace;
            font-size: 13px;
            margin-top: 20px;
        }}
        .footer {{
            margin-top: 25px;
            font-size: 12px;
            color: #8b949e;
        }}
    </style>
</head>
<body>
    <div class="card">
        <h1>Aktivitas Mencurigakan Terdeteksi</h1>
        <p>Request Anda telah diblokir secara otomatis oleh ZenoPanel Web Application Firewall (WAF) karena mengandung pola berbahaya.</p>
        <div class="details">
            <strong>IP Address:</strong> {}<br>
            <strong>Reason:</strong> {}<br>
            <strong>Timestamp:</strong> {}
        </div>
        <div class="footer">
            ZenoPanel Protection Engine &copy; 2026
        </div>
    </div>
</body>
</html>"#,
        ip, reason, chrono::Utc::now().to_rfc3339()
    )
}

fn render_rate_limited_page(ip: IpAddr) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Too Many Requests - ZenoPanel</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body {{
            font-family: 'Inter', system-ui, -apple-system, sans-serif;
            background-color: #0d1117;
            color: #c9d1d9;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
        }}
        .card {{
            background-color: #161b22;
            border: 1px solid #30363d;
            border-radius: 12px;
            padding: 40px;
            max-width: 500px;
            width: 90%;
            box-shadow: 0 8px 24px rgba(0,0,0,0.5);
            text-align: center;
        }}
        h1 {{
            color: #dbb32d;
            margin-top: 0;
            font-size: 24px;
        }}
        p {{
            line-height: 1.6;
            font-size: 15px;
        }}
        .footer {{
            margin-top: 25px;
            font-size: 12px;
            color: #8b949e;
        }}
    </style>
</head>
<body>
    <div class="card">
        <h1>Terlalu Banyak Request (429)</h1>
        <p>IP Anda ({}) mengirim terlalu banyak request dalam waktu singkat. Mohon tunggu beberapa saat sebelum mencoba kembali.</p>
        <div class="footer">
            ZenoPanel Protection Engine &copy; 2026
        </div>
    </div>
</body>
</html>"#,
        ip
    )
}

fn log_waf_to_db(db_manager: &crate::db::DBManager, ip: &str, reason: &str, target: &str) {
    let db_manager = db_manager.clone();
    let ip_str = ip.to_string();
    let reason_str = reason.to_string();
    let target_str = target.to_string();
    tokio::spawn(async move {
        if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
            let _ = sqlx::query("INSERT INTO waf_logs (ip, reason, target) VALUES (?, ?, ?)")
                .bind(ip_str)
                .bind(reason_str)
                .bind(target_str)
                .execute(&pool)
                .await;
        }
    });
}

pub fn write_access_log(ip: &str, method: &str, path: &str, status: u16, latency_ms: u64, bytes_sent: u64, user_agent: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let log_line = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "ip": ip,
        "method": method,
        "path": path,
        "status": status,
        "latency_ms": latency_ms,
        "bytes_sent": bytes_sent,
        "user_agent": user_agent
    });
    
    if let Ok(log_str) = serde_json::to_string(&log_line) {
        let mut file_path = std::path::PathBuf::from("logs");
        let _ = std::fs::create_dir_all(&file_path);
        file_path.push("access.log");
        
        if let Ok(mut file) = OpenOptions::new().create(true).write(true).append(true).open(file_path) {
            let _ = writeln!(file, "{}", log_str);
        }
    }
}

pub(crate) async fn waf_middleware(
    State(state): State<std::sync::Arc<crate::AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    req: Request,
    next: Next,
) -> Response {
    let start = std::time::Instant::now();
    let headers = req.headers().clone();
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let user_agent = headers.get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let ip = get_client_ip(&headers, connect_info.as_ref());
    let ip_str = ip.to_string();

    let req_size = headers.get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // 1. Check Rate Limiting
    if !state.rate_limiter.check_limit(ip) {
        let html = render_rate_limited_page(ip);
        let status = StatusCode::TOO_MANY_REQUESTS;
        
        log_waf_to_db(&state.db_manager, &ip_str, "Rate Limit Exceeded", &path);

        let latency = start.elapsed().as_millis() as u64;
        let bytes_sent = html.len() as u64;
        state.traffic_stats.record(req_size, bytes_sent, latency);

        let ua_clone = user_agent.clone();
        let ip_clone = ip_str.clone();
        let method_clone = method.clone();
        let path_clone = path.clone();
        tokio::spawn(async move {
            write_access_log(&ip_clone, &method_clone, &path_clone, status.as_u16(), latency, bytes_sent, &ua_clone);
        });

        return Response::builder()
            .status(status)
            .header("Content-Type", "text/html")
            .body(Body::from(html))
            .unwrap();
    }

    // 2. Check WAF
    if state.waf_enabled.load(std::sync::atomic::Ordering::Relaxed) {
        let uri = req.uri();
        let mut block_reason = None;

        if let Some(reason) = is_malicious(uri.path()) {
            block_reason = Some(reason);
        }
        
        if block_reason.is_none() {
            if let Some(query) = uri.query() {
                if let Some(reason) = is_malicious(query) {
                    block_reason = Some(reason);
                }
            }
        }

        if block_reason.is_none() {
            for (name, value) in headers.iter() {
                let name_str = name.as_str();
                if name_str == "user-agent" || name_str == "cookie" || name_str == "referer" {
                    if let Ok(val_str) = value.to_str() {
                        if let Some(reason) = is_malicious(val_str) {
                            block_reason = Some(reason);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(reason) = block_reason {
            let html = render_blocked_page(reason, ip);
            let status = StatusCode::FORBIDDEN;

            log_waf_to_db(&state.db_manager, &ip_str, reason, &path);

            let latency = start.elapsed().as_millis() as u64;
            let bytes_sent = html.len() as u64;
            state.traffic_stats.record(req_size, bytes_sent, latency);

            let ua_clone = user_agent.clone();
            let ip_clone = ip_str.clone();
            let method_clone = method.clone();
            let path_clone = path.clone();
            tokio::spawn(async move {
                write_access_log(&ip_clone, &method_clone, &path_clone, status.as_u16(), latency, bytes_sent, &ua_clone);
            });

            return Response::builder()
                .status(status)
                .header("Content-Type", "text/html")
                .body(Body::from(html))
                .unwrap();
        }
    }

    // 3. Process request
    let response = next.run(req).await;
    
    let latency = start.elapsed().as_millis() as u64;
    let res_size = response.headers().get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    
    state.traffic_stats.record(req_size, res_size, latency);

    let status_code = response.status().as_u16();
    tokio::spawn(async move {
        write_access_log(&ip_str, &method, &path, status_code, latency, res_size, &user_agent);
    });

    response
}
