use std::sync::Arc;
use async_trait::async_trait;
use pingora::prelude::*;
use pingora::proxy::{ProxyHttp, Session};
use pingora::http::ResponseHeader;
use axum::http::StatusCode;
use serde_json::json;
use crate::AppState;

pub struct GatewayCtx {
    pub is_proxy_rule: bool,
    pub target_host: String,
    pub client_ip: std::net::IpAddr,
    pub strip_path: bool,
    pub rule_path: String,
}

pub struct ZenoGateway {
    state: Arc<AppState>,
}

impl ZenoGateway {
    pub(crate) fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

fn is_realtime_request(req_header: &pingora::http::RequestHeader) -> bool {
    let is_websocket = req_header
        .headers
        .get("Upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let is_sse = req_header
        .headers
        .get("Accept")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/event-stream"))
        .unwrap_or(false);

    is_websocket || is_sse
}

fn configure_peer(peer: &mut HttpPeer, is_realtime: bool) {
    peer.options.connection_timeout = Some(std::time::Duration::from_secs(10));
    peer.options.idle_timeout = Some(std::time::Duration::from_secs(60));
    
    if is_realtime {
        // Allow up to 1 hour of idle time for WebSockets and Server-Sent Events (SSE)
        peer.options.read_timeout = Some(std::time::Duration::from_secs(3600));
        peer.options.write_timeout = Some(std::time::Duration::from_secs(3600));
    } else {
        // Fast timeouts for standard HTTP requests
        peer.options.read_timeout = Some(std::time::Duration::from_secs(15));
        peer.options.write_timeout = Some(std::time::Duration::from_secs(15));
    }
}

#[async_trait]
impl ProxyHttp for ZenoGateway {
    type CTX = GatewayCtx;

    fn new_ctx(&self) -> Self::CTX {
        GatewayCtx {
            is_proxy_rule: false,
            target_host: String::new(),
            client_ip: std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            strip_path: false,
            rule_path: String::new(),
        }
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        // 1. Get client IP
        if let Some(addr) = session.client_addr() {
            if let Some(inet) = addr.as_inet() {
                ctx.client_ip = inet.ip();
            }
        }

        let req_header = session.req_header();
        let path = req_header.uri.path();

        // 1.5 Health Check Endpoint for Load Balancer
        if path == "/health" {
            let _rules = self.state.proxy_manager.list_rules().await;
            let db_healthy = true;
            
            let status = if db_healthy { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
            let status_str = if db_healthy { "healthy" } else { "unhealthy" };
            let body_json = json!({
                "status": status_str,
                "node": "zenopanel-gateway",
                "db_connected": db_healthy,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            let body_str = body_json.to_string();

            let mut resp = ResponseHeader::build(status, Some(1))?;
            resp.insert_header("Content-Type", "application/json")?;
            session.write_response_header(Box::new(resp), false).await?;
            session.write_response_body(Some(body_str.into()), true).await?;

            return Ok(true);
        }

        // 2. Rate Limiting Check
        if !self.state.rate_limiter.check_limit(ctx.client_ip) {
            let html = format!(
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
                ctx.client_ip
            );

            // Log rate limit
            let ip_str = ctx.client_ip.to_string();
            let path_str = path.to_string();
            let db_manager = self.state.db_manager.clone();
            tokio::spawn(async move {
                if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                    let _ = sqlx::query("INSERT INTO waf_logs (ip, reason, target) VALUES (?, ?, ?)")
                        .bind(ip_str)
                        .bind("Rate Limit Exceeded")
                        .bind(path_str)
                        .execute(&pool)
                        .await;
                }
            });

            // Return 429 response
            let mut resp = ResponseHeader::build(StatusCode::TOO_MANY_REQUESTS, Some(1))?;
            resp.insert_header("Content-Type", "text/html")?;
            session.write_response_header(Box::new(resp), false).await?;
            session.write_response_body(Some(html.into()), true).await?;

            return Ok(true);
        }

        // 3. WAF Check
        if self.state.waf_enabled.load(std::sync::atomic::Ordering::Relaxed) {
            let mut block_reason = None;

            if let Some(reason) = crate::waf::is_malicious(path) {
                block_reason = Some(reason);
            }

            if block_reason.is_none() {
                if let Some(query) = req_header.uri.query() {
                    if let Some(reason) = crate::waf::is_malicious(query) {
                        block_reason = Some(reason);
                    }
                }
            }

            if block_reason.is_none() {
                for (name, value) in req_header.headers.iter() {
                    let name_str = name.as_str();
                    if name_str == "user-agent" || name_str == "cookie" || name_str == "referer" {
                        if let Ok(val_str) = value.to_str() {
                            if let Some(reason) = crate::waf::is_malicious(val_str) {
                                block_reason = Some(reason);
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(reason) = block_reason {
                let html = format!(
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
                    ctx.client_ip, reason, chrono::Utc::now().to_rfc3339()
                );

                // Log WAF to DB
                let ip_str = ctx.client_ip.to_string();
                let path_str = path.to_string();
                let db_manager = self.state.db_manager.clone();
                let reason_str = reason.to_string();
                tokio::spawn(async move {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let _ = sqlx::query("INSERT INTO waf_logs (ip, reason, target) VALUES (?, ?, ?)")
                            .bind(ip_str)
                            .bind(reason_str)
                            .bind(path_str)
                            .execute(&pool)
                            .await;
                    }
                });

                // Return 403 Forbidden response
                let mut resp = ResponseHeader::build(StatusCode::FORBIDDEN, Some(1))?;
                resp.insert_header("Content-Type", "text/html")?;
                session.write_response_header(Box::new(resp), false).await?;
                session.write_response_body(Some(html.into()), true).await?;

                return Ok(true);
            }
        }

        // 4. Managed Process Status Check for Proxy Rules
        let host_header = req_header
            .headers
            .get("Host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let (host, request_port) = crate::proxyman::parse_host_port(host_header);
        let request_port = request_port.unwrap_or(80);

        if let Some(rule) = self.state.proxy_manager.match_rule(&host, request_port, path).await {
            if let Some(ref proc_id) = rule.managed_process_id {
                if !proc_id.is_empty() {
                    if let Some(status) = self.state.process_manager.get_process_status(proc_id).await {
                        if status != "running" {
                            let html = crate::render_error_page(
                                &self.state.engine,
                                &status,
                                &rule.name,
                                &format!("The linked process ({}) has status: {}", proc_id, status),
                            );

                            let mut resp = ResponseHeader::build(StatusCode::SERVICE_UNAVAILABLE, Some(1))?;
                            resp.insert_header("Content-Type", "text/html")?;
                            session.write_response_header(Box::new(resp), false).await?;
                            session.write_response_body(Some(html.into()), true).await?;

                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    async fn upstream_peer(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        let req_header = session.req_header();
        let host_header = req_header
            .headers
            .get("Host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        
        let (host, request_port) = crate::proxyman::parse_host_port(host_header);
        let request_port = request_port.unwrap_or(80);
        let path = req_header.uri.path();

        let is_realtime = is_realtime_request(req_header);

        if let Some(rule) = self.state.proxy_manager.match_rule(&host, request_port, path).await {
            if rule.rule_type == "static" {
                // Forward static site requests to local Axum instance
                ctx.is_proxy_rule = false;
                ctx.target_host = "127.0.0.1".to_string();

                let mut peer = HttpPeer::new(
                    format!("127.0.0.1:{}", self.state.mgmt_port),
                    false,
                    String::new(),
                );
                configure_peer(&mut peer, is_realtime);
                return Ok(Box::new(peer));
            }

            let target = self.state.proxy_manager.get_next_target(&rule.id, &rule.target).await;
            
            let is_https = target.starts_with("https://");
            let cleaned = target.trim_start_matches("http://").trim_start_matches("https://");
            let (t_host, t_port) = crate::proxyman::parse_host_port(cleaned);
            let t_port = t_port.unwrap_or(if is_https { 443 } else { 80 });

            ctx.is_proxy_rule = true;
            ctx.target_host = t_host.clone();
            ctx.strip_path = rule.strip_path;
            ctx.rule_path = rule.path.clone();

            let addr_str = format!("{}:{}", t_host, t_port);
            let socket_addr = match tokio::net::lookup_host(&addr_str).await {
                Ok(mut addrs) => {
                    if let Some(addr) = addrs.next() {
                        addr
                    } else {
                        return Err(pingora::Error::explain(
                            pingora::prelude::ErrorType::ConnectError,
                            format!("DNS lookup returned no addresses for {}", addr_str),
                        ));
                    }
                }
                Err(e) => {
                    return Err(pingora::Error::explain(
                        pingora::prelude::ErrorType::ConnectError,
                        format!("DNS lookup failed for {}: {}", addr_str, e),
                    ));
                }
            };

            let mut peer = HttpPeer::new(
                socket_addr,
                is_https,
                t_host.clone(),
            );
            configure_peer(&mut peer, is_realtime);
            Ok(Box::new(peer))
        } else {
            // Forward to local Axum instance
            ctx.is_proxy_rule = false;
            ctx.target_host = "127.0.0.1".to_string();

            let mut peer = HttpPeer::new(
                format!("127.0.0.1:{}", self.state.mgmt_port),
                false,
                String::new(),
            );
            configure_peer(&mut peer, is_realtime);
            Ok(Box::new(peer))
        }
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        if ctx.is_proxy_rule && !ctx.target_host.is_empty() {
            if ctx.strip_path {
                let current_path = upstream_request.uri.path();
                let stripped_path = current_path.strip_prefix(&ctx.rule_path).unwrap_or(current_path);
                let mut final_path = if stripped_path.is_empty() {
                    "/".to_string()
                } else {
                    stripped_path.to_string()
                };
                if !final_path.starts_with('/') {
                    final_path = format!("/{}", final_path);
                }

                let query = upstream_request.uri.query().unwrap_or("");
                let new_path_and_query = if query.is_empty() {
                    final_path
                } else {
                    format!("{}?{}", final_path, query)
                };

                if let Ok(pq) = new_path_and_query.parse::<axum::http::uri::PathAndQuery>() {
                    let mut parts = upstream_request.uri.clone().into_parts();
                    parts.path_and_query = Some(pq);
                    if let Ok(new_uri) = axum::http::uri::Uri::from_parts(parts) {
                        upstream_request.set_uri(new_uri);
                    }
                }
            }
        }

        let ip_str = ctx.client_ip.to_string();
        if let Some(xff) = upstream_request.headers.get("X-Forwarded-For") {
            if let Ok(val) = xff.to_str() {
                let new_xff = format!("{}, {}", val, ip_str);
                upstream_request.insert_header("X-Forwarded-For", &new_xff)?;
            } else {
                upstream_request.insert_header("X-Forwarded-For", &ip_str)?;
            }
        } else {
            upstream_request.insert_header("X-Forwarded-For", &ip_str)?;
        }

        let is_tls = session.digest().and_then(|d| d.ssl_digest.as_ref()).is_some();
        let proto = if is_tls { "https" } else { "http" };
        upstream_request.insert_header("X-Forwarded-Proto", proto)?;

        Ok(())
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        if ctx.is_proxy_rule {
            let req_header = session.req_header();
            if let Some(origin) = req_header.headers.get("Origin") {
                if let Ok(origin_str) = origin.to_str() {
                    // Echo dynamic CORS headers back to client
                    let _ = upstream_response.insert_header("Access-Control-Allow-Origin", origin_str);
                    let _ = upstream_response.insert_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS, HEAD");
                    let _ = upstream_response.insert_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Requested-With, X-CSRF-Token");
                    let _ = upstream_response.insert_header("Access-Control-Allow-Credentials", "true");
                }
            }
        }
        Ok(())
    }
}
