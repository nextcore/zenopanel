use std::sync::Arc;
use async_trait::async_trait;
use pingora::prelude::*;
use pingora::proxy::{ProxyHttp, Session};
use pingora::http::ResponseHeader;
use axum::http::StatusCode;
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

        if let Some(rule) = self.state.proxy_manager.match_rule(&host, request_port, path).await {
            if rule.rule_type == "static" {
                // Forward static site requests to local Axum instance
                ctx.is_proxy_rule = false;
                ctx.target_host = "127.0.0.1".to_string();

                let peer = Box::new(HttpPeer::new(
                    format!("127.0.0.1:{}", self.state.mgmt_port),
                    false,
                    String::new(),
                ));
                return Ok(peer);
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

            let peer = Box::new(HttpPeer::new(
                socket_addr,
                is_https,
                t_host.clone(),
            ));
            Ok(peer)
        } else {
            // Forward to local Axum instance
            ctx.is_proxy_rule = false;
            ctx.target_host = "127.0.0.1".to_string();

            let peer = Box::new(HttpPeer::new(
                format!("127.0.0.1:{}", self.state.mgmt_port),
                false,
                String::new(),
            ));
            Ok(peer)
        }
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        if ctx.is_proxy_rule && !ctx.target_host.is_empty() {
            upstream_request.insert_header("Host", &ctx.target_host)?;

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

        let proto = "http";
        upstream_request.insert_header("X-Forwarded-Proto", proto)?;

        Ok(())
    }
}
