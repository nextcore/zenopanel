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

// ── RateLimiter ──────────────────────────────────────────────────────────────

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

    pub fn prune_old_entries(&self) {
        let now = Instant::now();
        let window = *self.window.lock().unwrap();
        let cutoff = now.checked_sub(window).unwrap_or(now);
        let mut reqs = self.requests.lock().unwrap();
        reqs.retain(|_, instants| {
            instants.retain(|&t| t > cutoff);
            !instants.is_empty()
        });
    }
}

// ── IpBlockList ──────────────────────────────────────────────────────────────

/// Live-updateable IP block/whitelist. "block" = denied, "allow" = whitelisted (bypasses WAF).
pub struct IpBlockList {
    entries: Mutex<HashMap<IpAddr, String>>, // ip -> "block" | "allow"
}

impl IpBlockList {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Add or update an IP entry. `rule_type` is "block" or "allow".
    pub fn add(&self, ip_str: &str, rule_type: &str) {
        if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
            let mut map = self.entries.lock().unwrap();
            map.insert(ip, rule_type.to_string());
        }
    }

    pub fn remove(&self, ip_str: &str) {
        if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
            self.entries.lock().unwrap().remove(&ip);
        }
    }

    /// Returns Some("block") if IP is blocked, Some("allow") if whitelisted, None if not in list.
    pub fn check(&self, ip: &IpAddr) -> Option<String> {
        self.entries.lock().unwrap().get(ip).cloned()
    }

    pub fn list(&self) -> Vec<(String, String)> {
        self.entries.lock().unwrap()
            .iter()
            .map(|(ip, t)| (ip.to_string(), t.clone()))
            .collect()
    }
}

// ── TrafficStatsManager ───────────────────────────────────────────────────────

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

// ── IP helper ────────────────────────────────────────────────────────────────

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

// ── Detection patterns ────────────────────────────────────────────────────────

/// SQL Injection — covers UNION-based, boolean-based, time-based blind, stacked queries
static SQLI_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(union[\s\+/\*]+(?:all[\s\+/\*]+)?select|select[\s\S]+from[\s\S]+where|insert[\s\+]+into|delete[\s\+]+from|drop[\s\+]+(?:table|database|schema)|update[\s\S]+set[\s\S]+where|or[\s\+]+[\d']+[\s\+]*=[\s\+]*[\d']+|and[\s\+]+[\d']+[\s\+]*=[\s\+]*[\d']+|--|#|/\*[\s\S]*?\*/|xp_cmdshell|information_schema|sleep\(\s*\d+\s*\)|benchmark\(|waitfor[\s\+]+delay|load_file\(|into[\s\+]+outfile|group[\s\+]+by[\s\+]+.*having)"
    ).unwrap()
});

/// XSS — covers reflected, DOM, attribute injection, data URIs
static XSS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)(<script[\s>]|<\/script>|javascript\s*:|vbscript\s*:|on\w+\s*=\s*['"]?[^'">\s]|alert\s*\(|confirm\s*\(|prompt\s*\(|document\.cookie|document\.write|window\.location|<iframe|<object|<svg[\s>]|<embed|<link\s+rel\s*=\s*['"]stylesheet['"]|data\s*:\s*text/html|eval\s*\(|<form\s+action\s*=)"#
    ).unwrap()
});

/// Path Traversal — covers plain, encoded, double-encoded
static PATH_TRAVERSAL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(\.\./|\.\.\\|%2e%2e[%/\\]|%252e%252e|\.%2e|%2e\.|/etc/passwd|/etc/shadow|/etc/hosts|/proc/self|/win\.ini|/boot\.ini|\x00)"
    ).unwrap()
});

/// Remote Code Execution — shell commands, PHP code injection, template injection
static RCE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(/bin/(?:bash|sh|zsh|ksh)|cmd\.exe|powershell\.exe|curl\s+[hf]|wget\s+[hf]|nc\s+-|netcat\s|sh\s+-c\s|bash\s+-c\s|exec\s*\(|system\s*\(|passthru\s*\(|shell_exec\s*\(|popen\s*\(|eval\s*\(|base64_decode\s*\(|phpinfo\s*\(|\{\{.*\}\}|\$\{.*\}|`[^`]+`)"
    ).unwrap()
});

/// SSRF — attempts to reach internal/cloud metadata endpoints
static SSRF_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(127\.0\.0\.1|localhost|0\.0\.0\.0|::1|169\.254\.169\.254|metadata\.google\.internal|100\.100\.100\.200|192\.168\.\d+\.\d+|10\.\d+\.\d+\.\d+|172\.(1[6-9]|2\d|3[01])\.\d+\.\d+|file://|gopher://|dict://|ftp://internal|sftp://)"
    ).unwrap()
});

/// Log4Shell — JNDI injection variants
static LOG4SHELL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(\$\{jndi:|%24%7bjndi:|%24%7Bjndi:|%24\{jndi|j%6endi|j%6Endi|\$\{.*lower.*:j.*ndi)"
    ).unwrap()
});

/// Scanner/Attack tool User-Agents
static SCANNER_UA_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(sqlmap|nikto|nmap|masscan|zgrab|nuclei|acunetix|nessus|openvas|burpsuite|dirbuster|gobuster|wfuzz|hydra|medusa|metasploit|havij|pangolin|havij|appscan|w3af|skipfish|arachni|grabber|vega|zap|whatweb|xsser|commix|beef/|dnsrecon|shodan|censys|zmap)"
    ).unwrap()
});

pub struct WafMatch {
    pub reason: &'static str,
    pub severity: &'static str,
}

pub fn is_malicious(input: &str) -> Option<WafMatch> {
    // Decode percent encoding to catch evasion attempts
    let decoded = urlencoding::decode(input)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|_| input.to_string());

    if LOG4SHELL_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "Log4Shell / JNDI Injection", severity: "critical" });
    }
    if SQLI_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "SQL Injection Pattern", severity: "high" });
    }
    if XSS_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "Cross-Site Scripting (XSS) Pattern", severity: "high" });
    }
    if RCE_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "Remote Code Execution Pattern", severity: "critical" });
    }
    if PATH_TRAVERSAL_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "Path Traversal Pattern", severity: "high" });
    }
    if SSRF_REGEX.is_match(&decoded) {
        return Some(WafMatch { reason: "Server-Side Request Forgery (SSRF) Pattern", severity: "high" });
    }
    None
}

pub fn is_scanner_bot(user_agent: &str) -> bool {
    SCANNER_UA_REGEX.is_match(user_agent)
}

// ── Block page rendering ──────────────────────────────────────────────────────

pub fn render_blocked_page(reason: &str, ip: IpAddr) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <title>Request Blocked - ZenoPanel WAF</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="robots" content="noindex">
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
        *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: 'Inter', system-ui, -apple-system, sans-serif;
            background: radial-gradient(ellipse at 50% 0%, #1a0a0a 0%, #0a0a0f 60%);
            color: #c9d1d9;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 20px;
        }}
        .shield {{
            width: 72px;
            height: 72px;
            background: linear-gradient(135deg, #f85149 0%, #da3633 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 32px;
            margin-bottom: 24px;
            box-shadow: 0 0 40px rgba(248, 81, 73, 0.4);
            animation: pulse 2s infinite;
        }}
        @keyframes pulse {{
            0%, 100% {{ box-shadow: 0 0 40px rgba(248, 81, 73, 0.4); }}
            50% {{ box-shadow: 0 0 60px rgba(248, 81, 73, 0.7); }}
        }}
        .card {{
            background: rgba(22, 27, 34, 0.85);
            backdrop-filter: blur(16px);
            border: 1px solid rgba(248, 81, 73, 0.25);
            border-radius: 16px;
            padding: 44px 48px;
            max-width: 520px;
            width: 100%;
            box-shadow: 0 24px 64px rgba(0,0,0,0.6);
            text-align: center;
        }}
        h1 {{
            color: #f85149;
            font-size: 22px;
            font-weight: 700;
            margin-bottom: 12px;
            letter-spacing: -0.3px;
        }}
        p {{
            line-height: 1.7;
            font-size: 14px;
            color: #8b949e;
            margin-bottom: 24px;
        }}
        .details {{
            background: rgba(13, 17, 23, 0.8);
            border: 1px solid rgba(48, 54, 61, 0.8);
            padding: 16px 20px;
            border-radius: 10px;
            text-align: left;
            font-family: 'JetBrains Mono', 'Fira Code', monospace;
            font-size: 12.5px;
            margin-bottom: 8px;
            line-height: 1.9;
        }}
        .details span {{ color: #58a6ff; font-weight: 600; }}
        .badge {{
            display: inline-block;
            background: rgba(248, 81, 73, 0.15);
            border: 1px solid rgba(248, 81, 73, 0.4);
            color: #f85149;
            font-size: 11px;
            font-weight: 600;
            padding: 3px 10px;
            border-radius: 20px;
            text-transform: uppercase;
            letter-spacing: 0.8px;
            margin-bottom: 20px;
        }}
        .footer {{
            margin-top: 28px;
            font-size: 11.5px;
            color: #484f58;
        }}
    </style>
</head>
<body>
    <div class="shield">🛡️</div>
    <div class="card">
        <div class="badge">403 Forbidden</div>
        <h1>Aktivitas Mencurigakan Terdeteksi</h1>
        <p>Request Anda telah diblokir secara otomatis oleh ZenoPanel Web Application Firewall karena mengandung pola yang berpotensi berbahaya.</p>
        <div class="details">
            <span>IP Address:</span> {ip}<br>
            <span>Threat:</span> {reason}<br>
            <span>Timestamp:</span> {ts}
        </div>
        <div class="footer">ZenoPanel Protection Engine &copy; 2026</div>
    </div>
</body>
</html>"#,
        ip = ip,
        reason = reason,
        ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
    )
}

pub fn render_rate_limited_page(ip: IpAddr) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <title>Too Many Requests - ZenoPanel</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="robots" content="noindex">
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
        *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: 'Inter', system-ui, -apple-system, sans-serif;
            background: radial-gradient(ellipse at 50% 0%, #0f0e00 0%, #0a0a0f 60%);
            color: #c9d1d9;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 20px;
        }}
        .icon {{
            width: 72px;
            height: 72px;
            background: linear-gradient(135deg, #dbb32d 0%, #b08800 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 32px;
            margin-bottom: 24px;
            box-shadow: 0 0 40px rgba(219, 179, 45, 0.4);
            animation: pulse 2s infinite;
        }}
        @keyframes pulse {{
            0%, 100% {{ box-shadow: 0 0 40px rgba(219, 179, 45, 0.4); }}
            50% {{ box-shadow: 0 0 60px rgba(219, 179, 45, 0.7); }}
        }}
        .card {{
            background: rgba(22, 27, 34, 0.85);
            backdrop-filter: blur(16px);
            border: 1px solid rgba(219, 179, 45, 0.25);
            border-radius: 16px;
            padding: 44px 48px;
            max-width: 520px;
            width: 100%;
            box-shadow: 0 24px 64px rgba(0,0,0,0.6);
            text-align: center;
        }}
        h1 {{
            color: #dbb32d;
            font-size: 22px;
            font-weight: 700;
            margin-bottom: 12px;
            letter-spacing: -0.3px;
        }}
        p {{
            line-height: 1.7;
            font-size: 14px;
            color: #8b949e;
        }}
        .badge {{
            display: inline-block;
            background: rgba(219, 179, 45, 0.12);
            border: 1px solid rgba(219, 179, 45, 0.4);
            color: #dbb32d;
            font-size: 11px;
            font-weight: 600;
            padding: 3px 10px;
            border-radius: 20px;
            text-transform: uppercase;
            letter-spacing: 0.8px;
            margin-bottom: 20px;
        }}
        .ip {{
            font-family: monospace;
            color: #58a6ff;
            font-weight: 600;
        }}
        .footer {{
            margin-top: 28px;
            font-size: 11.5px;
            color: #484f58;
        }}
    </style>
</head>
<body>
    <div class="icon">⏱️</div>
    <div class="card">
        <div class="badge">429 Too Many Requests</div>
        <h1>Terlalu Banyak Request</h1>
        <p>IP <span class="ip">{ip}</span> mengirim terlalu banyak request dalam waktu singkat. Mohon tunggu beberapa saat sebelum mencoba kembali.</p>
        <div class="footer">ZenoPanel Protection Engine &copy; 2026</div>
    </div>
</body>
</html>"#,
        ip = ip,
    )
}

pub fn render_blocked_ip_page(ip: IpAddr) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="id">
<head>
    <title>Access Denied - ZenoPanel</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="robots" content="noindex">
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
        *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: 'Inter', system-ui, -apple-system, sans-serif;
            background: radial-gradient(ellipse at 50% 0%, #0a0a14 0%, #0a0a0f 60%);
            color: #c9d1d9;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            padding: 20px;
        }}
        .icon {{
            width: 72px;
            height: 72px;
            background: linear-gradient(135deg, #8b5cf6 0%, #6d28d9 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 32px;
            margin-bottom: 24px;
            box-shadow: 0 0 40px rgba(139, 92, 246, 0.4);
        }}
        .card {{
            background: rgba(22, 27, 34, 0.85);
            backdrop-filter: blur(16px);
            border: 1px solid rgba(139, 92, 246, 0.25);
            border-radius: 16px;
            padding: 44px 48px;
            max-width: 520px;
            width: 100%;
            box-shadow: 0 24px 64px rgba(0,0,0,0.6);
            text-align: center;
        }}
        h1 {{ color: #8b5cf6; font-size: 22px; font-weight: 700; margin-bottom: 12px; }}
        p {{ line-height: 1.7; font-size: 14px; color: #8b949e; }}
        .badge {{
            display: inline-block;
            background: rgba(139, 92, 246, 0.12);
            border: 1px solid rgba(139, 92, 246, 0.4);
            color: #8b5cf6;
            font-size: 11px; font-weight: 600; padding: 3px 10px;
            border-radius: 20px; text-transform: uppercase; letter-spacing: 0.8px; margin-bottom: 20px;
        }}
        .ip {{ font-family: monospace; color: #58a6ff; font-weight: 600; }}
        .footer {{ margin-top: 28px; font-size: 11.5px; color: #484f58; }}
    </style>
</head>
<body>
    <div class="icon">🚫</div>
    <div class="card">
        <div class="badge">403 Access Denied</div>
        <h1>Akses Ditolak</h1>
        <p>IP <span class="ip">{ip}</span> tidak diijinkan mengakses layanan ini. Jika Anda yakin ini adalah kesalahan, hubungi administrator.</p>
        <div class="footer">ZenoPanel Protection Engine &copy; 2026</div>
    </div>
</body>
</html>"#,
        ip = ip,
    )
}

// ── DB logging ────────────────────────────────────────────────────────────────

fn log_waf_to_db(db_manager: &crate::db::DBManager, ip: &str, method: &str, reason: &str, severity: &str, target: &str) {
    let db_manager = db_manager.clone();
    let ip_str = ip.to_string();
    let method_str = method.to_string();
    let reason_str = reason.to_string();
    let severity_str = severity.to_string();
    let target_str = target.to_string();
    tokio::spawn(async move {
        if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
            let _ = sqlx::query("INSERT INTO waf_logs (ip, method, reason, severity, target) VALUES (?, ?, ?, ?, ?)")
                .bind(ip_str)
                .bind(method_str)
                .bind(reason_str)
                .bind(severity_str)
                .bind(target_str)
                .execute(&pool)
                .await;
        }
    });
}

// ── Access log ────────────────────────────────────────────────────────────────

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

// ── WAF Middleware ────────────────────────────────────────────────────────────

pub(crate) async fn waf_middleware(
    State(state): State<std::sync::Arc<crate::AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    req: Request,
    next: Next,
) -> Response {
    let start = std::time::Instant::now();
    let (parts, body) = req.into_parts();
    let headers = parts.headers.clone();
    let method = parts.method.as_str().to_string();
    let path = parts.uri.path().to_string();
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

    // 0. IP whitelist / blocklist check (highest priority)
    if let Some(rule_type) = state.ip_block_list.check(&ip) {
        if rule_type == "block" {
            let html = render_blocked_ip_page(ip);
            let status = StatusCode::FORBIDDEN;
            log_waf_to_db(&state.db_manager, &ip_str, &method, "IP Blocked by Administrator", "critical", &path);
            let latency = start.elapsed().as_millis() as u64;
            let bytes_sent = html.len() as u64;
            state.traffic_stats.record(req_size, bytes_sent, latency);
            return Response::builder()
                .status(status)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Body::from(html))
                .unwrap();
        }
        // "allow" → skip all WAF checks below, pass through
        let req = Request::from_parts(parts, body);
        let response = next.run(req).await;
        let latency = start.elapsed().as_millis() as u64;
        let res_size = response.headers().get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        state.traffic_stats.record(req_size, res_size, latency);
        let status_code = response.status().as_u16();
        let ua_c = user_agent.clone(); let ip_c = ip_str.clone(); let m_c = method.clone(); let p_c = path.clone();
        tokio::spawn(async move { write_access_log(&ip_c, &m_c, &p_c, status_code, latency, res_size, &ua_c); });
        return response;
    }

    // 1. Rate limiting
    if !state.rate_limiter.check_limit(ip) {
        let html = render_rate_limited_page(ip);
        let status = StatusCode::TOO_MANY_REQUESTS;
        log_waf_to_db(&state.db_manager, &ip_str, &method, "Rate Limit Exceeded", "medium", &path);
        let latency = start.elapsed().as_millis() as u64;
        let bytes_sent = html.len() as u64;
        state.traffic_stats.record(req_size, bytes_sent, latency);
        let ua_clone = user_agent.clone(); let ip_clone = ip_str.clone(); let method_clone = method.clone(); let path_clone = path.clone();
        tokio::spawn(async move { write_access_log(&ip_clone, &method_clone, &path_clone, status.as_u16(), latency, bytes_sent, &ua_clone); });
        return Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap();
    }

    // 2. WAF inspection
    let mut block_reason: Option<&'static str> = None;
    let mut block_severity: &'static str = "medium";
    let mut body_bytes = None;
    let mut body_opt = Some(body);

    if state.waf_enabled.load(std::sync::atomic::Ordering::Relaxed) {
        // 2a. Scanner bot detection via User-Agent
        if is_scanner_bot(&user_agent) {
            block_reason = Some("Known Attack Tool / Scanner Bot");
            block_severity = "high";
        }

        if block_reason.is_none() {
            let uri = parts.uri.clone();

            // 2b. Path
            if let Some(m) = is_malicious(uri.path()) {
                block_reason = Some(m.reason);
                block_severity = m.severity;
            }

            // 2c. Query string
            if block_reason.is_none() {
                if let Some(query) = uri.query() {
                    if let Some(m) = is_malicious(query) {
                        block_reason = Some(m.reason);
                        block_severity = m.severity;
                    }
                }
            }

            // 2d. Sensitive headers
            if block_reason.is_none() {
                for (name, value) in headers.iter() {
                    let name_str = name.as_str();
                    if name_str == "user-agent" || name_str == "referer" {
                        if let Ok(val_str) = value.to_str() {
                            if let Some(m) = is_malicious(val_str) {
                                block_reason = Some(m.reason);
                                block_severity = m.severity;
                                break;
                            }
                        }
                    } else if name_str == "cookie" {
                        if let Ok(val_str) = value.to_str() {
                            for pair in val_str.split(';') {
                                let pair = pair.trim();
                                let mut cookie_parts = pair.splitn(2, '=');
                                if let (Some(k), Some(v)) = (cookie_parts.next(), cookie_parts.next()) {
                                    let key = k.trim();
                                    if key == "zeno_token" || key == "_csrf" {
                                        continue;
                                    }
                                    if let Some(m) = is_malicious(v) {
                                        block_reason = Some(m.reason);
                                        block_severity = m.severity;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if block_reason.is_some() { break; }
                }
            }

            // 2e. Request body (skip admin/login paths)
            let entrance_path = {
                let lock = state.entrance_path.lock().unwrap();
                lock.clone()
            };
            let is_admin_or_login = path.starts_with("/api/") || path == entrance_path;

            if !is_admin_or_login && block_reason.is_none() && (method == "POST" || method == "PUT" || method == "PATCH") {
                if let Some(b) = body_opt.take() {
                    match axum::body::to_bytes(b, 2 * 1024 * 1024).await {
                        Ok(bytes) => {
                            let body_str = String::from_utf8_lossy(&bytes);
                            if let Some(m) = is_malicious(&body_str) {
                                block_reason = Some(m.reason);
                                block_severity = m.severity;
                            }
                            body_bytes = Some(bytes);
                        }
                        Err(_) => {
                            block_reason = Some("Request Body Read Error / Payload Too Large");
                            block_severity = "medium";
                            body_bytes = Some(axum::body::Bytes::new());
                        }
                    }
                }
            }
        }
    }

    if let Some(reason) = block_reason {
        let html = render_blocked_page(reason, ip);
        let status = StatusCode::FORBIDDEN;
        log_waf_to_db(&state.db_manager, &ip_str, &method, reason, block_severity, &path);
        let latency = start.elapsed().as_millis() as u64;
        let bytes_sent = html.len() as u64;
        state.traffic_stats.record(req_size, bytes_sent, latency);
        let ua_clone = user_agent.clone(); let ip_clone = ip_str.clone(); let method_clone = method.clone(); let path_clone = path.clone();
        tokio::spawn(async move { write_access_log(&ip_clone, &method_clone, &path_clone, status.as_u16(), latency, bytes_sent, &ua_clone); });
        return Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap();
    }

    // Reconstruct request and pass through
    let req = if let Some(bytes) = body_bytes {
        Request::from_parts(parts, Body::from(bytes))
    } else {
        Request::from_parts(parts, body_opt.take().unwrap())
    };

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

    // Add security response headers to all legitimate responses
    let (mut resp_parts, resp_body) = response.into_parts();
    {
        let h = &mut resp_parts.headers;
        h.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
        h.insert("X-Frame-Options", "SAMEORIGIN".parse().unwrap());
        h.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
        h.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    }
    Response::from_parts(resp_parts, resp_body)
}
