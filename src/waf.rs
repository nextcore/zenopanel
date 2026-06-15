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

pub struct RateLimiter {
    requests: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    enabled: bool,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(enabled: bool, max_requests: usize, window_secs: u64) -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            enabled,
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    pub fn check_limit(&self, ip: IpAddr) -> bool {
        if !self.enabled {
            return true;
        }
        let now = Instant::now();
        let mut reqs = self.requests.lock().unwrap();
        
        let ip_reqs = reqs.entry(ip).or_insert_with(Vec::new);
        let window_start = now.checked_sub(self.window).unwrap_or(now);
        ip_reqs.retain(|&t| t > window_start);

        if ip_reqs.len() >= self.max_requests {
            false
        } else {
            ip_reqs.push(now);
            true
        }
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

pub(crate) async fn waf_middleware(
    State(state): State<std::sync::Arc<crate::AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();
    let ip = get_client_ip(headers, connect_info.as_ref());

    // 1. Check Rate Limiting
    if !state.rate_limiter.check_limit(ip) {
        let html = render_rate_limited_page(ip);
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("Content-Type", "text/html")
            .body(Body::from(html))
            .unwrap();
    }

    // 2. Check WAF
    if state.waf_enabled {
        let uri = req.uri();
        
        // Scan path
        if let Some(reason) = is_malicious(uri.path()) {
            let html = render_blocked_page(reason, ip);
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header("Content-Type", "text/html")
                .body(Body::from(html))
                .unwrap();
        }
        
        // Scan query
        if let Some(query) = uri.query() {
            if let Some(reason) = is_malicious(query) {
                let html = render_blocked_page(reason, ip);
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("Content-Type", "text/html")
                    .body(Body::from(html))
                    .unwrap();
            }
        }

        // Scan header values
        for (name, value) in headers.iter() {
            let name_str = name.as_str();
            if name_str == "user-agent" || name_str == "cookie" || name_str == "referer" {
                if let Ok(val_str) = value.to_str() {
                    if let Some(reason) = is_malicious(val_str) {
                        let html = render_blocked_page(reason, ip);
                        return Response::builder()
                            .status(StatusCode::FORBIDDEN)
                            .header("Content-Type", "text/html")
                            .body(Body::from(html))
                            .unwrap();
                    }
                }
            }
        }
    }

    next.run(req).await
}
