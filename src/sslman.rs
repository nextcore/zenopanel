use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use rustls::server::{ResolvesServerCert, ClientHello};
use rustls::sign::CertifiedKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rcgen::generate_simple_self_signed;
use once_cell::sync::Lazy;
use crate::proxyman::ProxyManager;

// Global map to store active ACME challenge tokens and their responses
pub static ACME_CHALLENGES: Lazy<Arc<std::sync::Mutex<HashMap<String, String>>>> = 
    Lazy::new(|| Arc::new(std::sync::Mutex::new(HashMap::new())));

pub struct ZenoCertResolver {
    proxy_manager: Arc<ProxyManager>,
    cert_cache: Arc<RwLock<HashMap<String, Arc<CertifiedKey>>>>,
    certs_dir: String,
}

impl std::fmt::Debug for ZenoCertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZenoCertResolver")
            .field("certs_dir", &self.certs_dir)
            .finish()
    }
}

impl ZenoCertResolver {
    pub fn new(proxy_manager: Arc<ProxyManager>, certs_dir: &str) -> Self {
        // Ensure certs directory exists
        if !Path::new(certs_dir).exists() {
            let _ = std::fs::create_dir_all(certs_dir);
        }

        Self {
            proxy_manager,
            cert_cache: Arc::new(RwLock::new(HashMap::new())),
            certs_dir: certs_dir.to_string(),
        }
    }

    pub fn certs_dir(&self) -> &str {
        &self.certs_dir
    }

    pub async fn clear_cache(&self, domain: &str) {
        let mut cache = self.cert_cache.write().await;
        cache.remove(domain);
        println!("[SSL] Cleared in-memory cert cache for domain: {}", domain);
    }

    // Load cert & key from disk cache, or generate self-signed on demand
    pub async fn get_or_create_cert(&self, domain: &str) -> Option<Arc<CertifiedKey>> {
        // 1. Check in-memory cache
        {
            let cache = self.cert_cache.read().await;
            if let Some(cert) = cache.get(domain) {
                return Some(cert.clone());
            }
        }

        // 2. Check disk cache
        let cert_path = format!("{}/{}.crt", self.certs_dir, domain);
        let key_path = format!("{}/{}.key", self.certs_dir, domain);

        if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
            if let Ok(certs) = load_certs_from_file(Path::new(&cert_path)) {
                if let Ok(key) = load_private_key_from_file(Path::new(&key_path)) {
                    let provider = rustls::crypto::ring::default_provider();
                    if let Ok(certified_key) = CertifiedKey::from_der(certs, key, &provider) {
                        let arc_key = Arc::new(certified_key);
                        self.cert_cache.write().await.insert(domain.to_string(), arc_key.clone());
                        return Some(arc_key);
                    }
                }
            }
        }

        // 3. Check if SSL is enabled for this domain in proxy rules
        // If not found in DB or SSL is disabled, do not issue
        let rules = self.proxy_manager.list_rules().await;
        let rule = rules.iter().find(|r| {
            r.ssl_enabled && (r.domain == domain || (r.domain == "*" && domain == "localhost"))
        });

        if rule.is_none() {
            return None;
        }

        let rule = rule.unwrap();

        // 4. Generate self-signed certificate on demand (instant fallback)
        println!("[SSL] Generating self-signed certificate for domain: {}", domain);
        let subject_alt_names = vec![domain.to_string()];
        
        match generate_simple_self_signed(subject_alt_names) {
            Ok(rcgen_key) => {
                let cert_der = rcgen_key.cert.der().to_vec();
                let key_der = rcgen_key.key_pair.serialize_der();

                // Save to disk cache
                let _ = std::fs::write(&cert_path, &cert_der);
                let _ = std::fs::write(&key_path, &key_der);

                if let Ok(certs) = load_certs_from_file(Path::new(&cert_path)) {
                    if let Ok(key) = load_private_key_from_file(Path::new(&key_path)) {
                        let provider = rustls::crypto::ring::default_provider();
                        if let Ok(certified_key) = CertifiedKey::from_der(certs, key, &provider) {
                            let arc_key = Arc::new(certified_key);
                            self.cert_cache.write().await.insert(domain.to_string(), arc_key.clone());

                            // If status is currently pending or none, update status to self-signed
                            if rule.ssl_status == "pending" || rule.ssl_status == "none" {
                                let pm = self.proxy_manager.clone();
                                let rule_id = rule.id.clone();
                                let domain_str = domain.to_string();
                                tokio::spawn(async move {
                                    // Set status to active_self_signed, then trigger background ACME check if it's public
                                    let _ = pm.update_ssl_status(&rule_id, "active_self_signed").await;
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                                    trigger_acme_flow(pm, rule_id, domain_str).await;
                                });
                            }

                            return Some(arc_key);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[SSL] Failed to generate self-signed cert for {}: {}", domain, e);
            }
        }

        None
    }
}

// Helpers to load PEM/DER certificates and keys
fn load_certs_from_file(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open certificate file: {}", e))?;
    let mut reader = std::io::BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse PEM certificate chain: {}", e))?;
        
    if certs.is_empty() {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read certificate file bytes: {}", e))?;
        if !bytes.is_empty() {
            return Ok(vec![CertificateDer::from(bytes)]);
        }
        return Err("Certificate file is empty".to_string());
    }
    Ok(certs)
}

fn load_private_key_from_file(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open private key file: {}", e))?;
    let mut reader = std::io::BufReader::new(file);
    if let Some(key) = rustls_pemfile::private_key(&mut reader)
        .map_err(|e| format!("Failed to parse PEM private key: {}", e))? {
        Ok(key)
    } else {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read private key file bytes: {}", e))?;
        PrivateKeyDer::try_from(bytes)
            .map_err(|e| format!("Failed to parse private key as DER: {}", e))
    }
}

impl ResolvesServerCert for ZenoCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let name = client_hello.server_name()?;
        
        // Use block_in_place to await async get_or_create_cert in synchronous resolve method
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.get_or_create_cert(name).await
            })
        })
    }
}

// Background ACME flow trigger
async fn trigger_acme_flow(proxy_manager: Arc<ProxyManager>, rule_id: String, domain: String) {
    // Check if domain is local or public
    let is_local = domain == "localhost" 
        || domain == "127.0.0.1"
        || domain.ends_with(".local") 
        || domain.ends_with(".test") 
        || domain.ends_with(".lan");

    if is_local {
        // Local domains immediately finalize with self-signed certificate (active_self_signed)
        let _ = proxy_manager.update_ssl_status(&rule_id, "active_self_signed").await;
        println!("[SSL] Domain '{}' is local, finalized with self-signed certificate.", domain);
        return;
    }

    // For public domains, run real ACME Let's Encrypt HTTP-01 challenge
    println!("[SSL] Triggering Let's Encrypt HTTP-01 challenge for domain: {}", domain);
    let _ = proxy_manager.update_ssl_status(&rule_id, "pending").await;

    let pm = proxy_manager.clone();
    tokio::spawn(async move {
        match perform_acme_flow(pm.clone(), &rule_id, &domain).await {
            Ok(_) => {
                println!("[SSL] ACME certificate successfully provisioned for domain: {}", domain);
            }
            Err(e) => {
                eprintln!("[SSL] ACME certificate provisioning failed for domain {}: {}", domain, e);
                let _ = pm.update_ssl_status(&rule_id, "failed").await;
            }
        }
    });
}

// Actual ACME flow runner using instant-acme
async fn perform_acme_flow(proxy_manager: Arc<ProxyManager>, rule_id: &str, domain: &str) -> Result<(), String> {
    use instant_acme::{Account, NewAccount, NewOrder, Identifier, ChallengeType, AuthorizationStatus, LetsEncrypt, AccountCredentials};

    // 1. Determine directory URL based on SSL_PRODUCTION env var
    let production = std::env::var("SSL_PRODUCTION")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);
    let directory_url = if production {
        LetsEncrypt::Production.url().to_string()
    } else {
        LetsEncrypt::Staging.url().to_string()
    };

    println!("[SSL ACME] Using directory URL: {}", directory_url);

    // 2. Load or register ACME account using AccountCredentials
    let account_credentials_path = "./certs/acme_account.json";
    let contact_email = std::env::var("SSL_CONTACT_EMAIL")
        .unwrap_or_else(|_| "admin@zenopanel.local".to_string());
    let contact_uri = format!("mailto:{}", contact_email);

    let account = if std::path::Path::new(account_credentials_path).exists() {
        println!("[SSL ACME] Loading existing ACME account credentials from {}", account_credentials_path);
        let bytes = std::fs::read(account_credentials_path)
            .map_err(|e| format!("Failed to read ACME credentials file: {}", e))?;
        let creds: AccountCredentials = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Failed to parse ACME credentials: {}", e))?;
        Account::builder()
            .map_err(|e| format!("Failed to create Account builder: {}", e))?
            .from_credentials(creds)
            .await
            .map_err(|e| format!("Failed to restore ACME account: {}", e))?
    } else {
        println!("[SSL ACME] Registering new ACME account for: {}", contact_email);
        let (acc, creds) = Account::builder()
            .map_err(|e| format!("Failed to create Account builder: {}", e))?
            .create(
                &NewAccount {
                    contact: &[&contact_uri],
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                directory_url.clone(),
                None,
            )
            .await
            .map_err(|e| format!("ACME Account creation failed: {}", e))?;
        
        let bytes = serde_json::to_vec(&creds)
            .map_err(|e| format!("Failed to serialize ACME credentials: {}", e))?;
        if let Some(parent) = std::path::Path::new(account_credentials_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create certs directory: {}", e))?;
        }
        std::fs::write(account_credentials_path, bytes)
            .map_err(|e| format!("Failed to save ACME credentials: {}", e))?;
        acc
    };

    // 3. Create new order
    println!("[SSL ACME] Creating ACME order for: {}", domain);
    let idents = [Identifier::Dns(domain.to_string())];
    let new_order = NewOrder::new(&idents);
    let mut order = account.new_order(&new_order)
        .await
        .map_err(|e| format!("ACME Order creation failed: {}", e))?;

    // 4. Retrieve authorizations and find HTTP-01 challenge
    let mut challenge_token = None;
    let mut authorizations = order.authorizations();
    while let Some(auth_res) = authorizations.next().await {
        let mut auth = auth_res.map_err(|e| format!("Failed to retrieve ACME authorization: {}", e))?;
        if let Some(mut challenge_handle) = auth.challenge(ChallengeType::Http01) {
            let token = challenge_handle.token.clone();
            let key_auth = challenge_handle.key_authorization();
            let key_auth_str = key_auth.as_str().to_string();

            // Insert into the global challenges map
            {
                let mut challenges = ACME_CHALLENGES.lock().unwrap();
                challenges.insert(token.clone(), key_auth_str);
            }

            challenge_token = Some(token);

            println!("[SSL ACME] Fulfilling challenge for {}. Serving token: {} at /.well-known/acme-challenge/{}", 
                     domain, challenge_handle.token, challenge_handle.token);

            // Signal readiness to ACME server
            challenge_handle.set_ready()
                .await
                .map_err(|e| format!("Failed to trigger ACME challenge: {}", e))?;
            break;
        }
    }

    if challenge_token.is_none() {
        return Err("No HTTP-01 challenge found in ACME order".to_string());
    }

    // 5. Poll challenge status until it's valid or failed
    let mut success = false;
    for _ in 0..15 {
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        
        let mut auths = order.authorizations();
        let mut all_valid = true;
        let mut any_failed = false;
        
        while let Some(auth_res) = auths.next().await {
            let auth = auth_res.map_err(|e| format!("Failed to refresh authorization: {}", e))?;
            match auth.status {
                AuthorizationStatus::Valid => {},
                AuthorizationStatus::Invalid => {
                    any_failed = true;
                },
                _ => {
                    all_valid = false;
                }
            }
        }
        
        if all_valid {
            success = true;
            break;
        }
        
        if any_failed {
            // Clean up the challenge from the global map
            if let Some(ref tok) = challenge_token {
                let mut challenges = ACME_CHALLENGES.lock().unwrap();
                challenges.remove(tok);
            }
            return Err("ACME challenge validation failed on server".to_string());
        }
    }
    
    // Clean up the challenge from the global map
    if let Some(ref tok) = challenge_token {
        let mut challenges = ACME_CHALLENGES.lock().unwrap();
        challenges.remove(tok);
    }
    
    if !success {
        return Err("ACME challenge validation timed out".to_string());
    }

    // 6. Generate key pair and CSR for the domain
    println!("[SSL ACME] Generating key pair and CSR for: {}", domain);
    let mut params = rcgen::CertificateParams::new(vec![domain.to_string()])
        .map_err(|e| format!("CertificateParams creation failed: {}", e))?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(rcgen::DnType::CommonName, domain);
    
    let key_pair = rcgen::KeyPair::generate()
        .map_err(|e| format!("KeyPair generation failed: {}", e))?;
    
    let csr = params.serialize_request(&key_pair)
        .map_err(|e| format!("CSR serialization failed: {}", e))?;
    
    let csr_der = csr.der();

    println!("[SSL ACME] Finalizing ACME order...");
    order.finalize_csr(&csr_der)
        .await
        .map_err(|e| format!("Order finalization failed: {}", e))?;

    // 7. Poll order certificate status until it is Valid
    let mut certificate_pem = None;
    for _ in 0..15 {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        match order.certificate().await {
            Ok(Some(cert_pem)) => {
                certificate_pem = Some(cert_pem);
                break;
            }
            Ok(None) => {
                println!("[SSL ACME] Certificate order is still processing...");
            }
            Err(e) => {
                return Err(format!("Failed to retrieve certificate: {}", e));
            }
        }
    }

    let cert_pem = certificate_pem.ok_or_else(|| "ACME order certificate retrieval timed out".to_string())?;

    // 8. Save signed certificate (PEM) and private key (DER) to disk
    let cert_path = format!("./certs/{}.crt", domain);
    let key_path = format!("./certs/{}.key", domain);
    
    if let Some(parent) = std::path::Path::new(&cert_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create certs directory: {}", e))?;
    }

    std::fs::write(&cert_path, cert_pem.as_bytes())
        .map_err(|e| format!("Failed to save signed certificate: {}", e))?;
    std::fs::write(&key_path, key_pair.serialized_der())
        .map_err(|e| format!("Failed to save private key: {}", e))?;

    // 9. Update SSL status in database to active_letsencrypt
    let _ = proxy_manager.update_ssl_status(rule_id, "active_letsencrypt").await;
    println!("[SSL ACME] SSL configuration successfully completed for: {}", domain);

    Ok(())
}

// Check and renew all active SSL certificates if close to expiration (older than 60 days)
pub async fn check_and_renew_certs(proxy_manager: Arc<ProxyManager>, cert_resolver: Arc<ZenoCertResolver>) {
    println!("[SSL Renewal] Checking SSL certificates for active proxy rules...");
    let rules = proxy_manager.list_rules().await;
    
    for rule in rules {
        if !rule.ssl_enabled {
            continue;
        }
        
        let domain = &rule.domain;
        
        // Skip local domains since they use on-demand self-signed and don't need renewal checking
        let is_local = domain == "localhost" 
            || domain == "127.0.0.1"
            || domain.ends_with(".local") 
            || domain.ends_with(".test") 
            || domain.ends_with(".lan");
            
        if is_local {
            continue;
        }
        
        let cert_path = format!("{}/{}.crt", cert_resolver.certs_dir(), domain);
        
        let needs_renewal = if !Path::new(&cert_path).exists() {
            true
        } else {
            match load_certs_from_file(Path::new(&cert_path)) {
                Ok(certs) => {
                    if certs.is_empty() {
                        true
                    } else {
                        let der_bytes = certs[0].as_ref();
                        match x509_parser::parse_x509_certificate(der_bytes) {
                            Ok((_, x509)) => {
                                let validity = x509.validity();
                                if let Some(duration_until_expiry) = validity.time_to_expiration() {
                                    let secs = duration_until_expiry.whole_seconds();
                                    println!("[SSL Renewal] Cert for '{}' has {} seconds (approx {:.1} days) remaining.", domain, secs, secs as f64 / 86400.0);
                                    secs < 2592000
                                } else {
                                    println!("[SSL Renewal] Could not determine expiration duration for '{}'. Forcing renewal.", domain);
                                    true
                                }
                            }
                            Err(e) => {
                                println!("[SSL Renewal] Failed to parse X509 certificate for '{}': {:?}. Forcing renewal.", domain, e);
                                true
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[SSL Renewal] Failed to load certificate file for '{}': {}. Forcing renewal.", domain, e);
                    true
                }
            }
        };
        
        // Also retry if the status is failed or active_self_signed (meaning it fallback to self-signed but is a public domain)
        let is_fallback_or_failed = rule.ssl_status == "failed" || rule.ssl_status == "active_self_signed";
        
        if needs_renewal || is_fallback_or_failed {
            println!("[SSL Renewal] Cert for '{}' needs renewal (age check: {}, status: {}). Triggering ACME...", 
                     domain, needs_renewal, rule.ssl_status);
                     
            // Clear the in-memory cache for this domain
            cert_resolver.clear_cache(domain).await;
            
            // Trigger the ACME flow
            trigger_acme_flow(proxy_manager.clone(), rule.id.clone(), domain.clone()).await;
        }
    }
}

// Background auto-renewal worker running every 12 hours
pub async fn start_auto_renewal_worker(proxy_manager: Arc<ProxyManager>, cert_resolver: Arc<ZenoCertResolver>) {
    // Initial sleep to let the HTTP/HTTPS servers bind and load up
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    loop {
        check_and_renew_certs(proxy_manager.clone(), cert_resolver.clone()).await;
        
        // Sleep for 12 hours
        tokio::time::sleep(tokio::time::Duration::from_secs(12 * 3600)).await;
    }
}

// Start HTTPS/TLS server on port 443 (or configured APP_TLS_PORT)
pub async fn run_tls_server(cert_resolver: Arc<ZenoCertResolver>, app: axum::Router) {
    let tls_port = std::env::var("APP_TLS_PORT")
        .unwrap_or_else(|_| ":8443".to_string())
        .trim_start_matches(':')
        .parse::<u16>()
        .unwrap_or(8443);

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(cert_resolver);
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    let listener = match TcpListener::bind(format!("0.0.0.0:{}", tls_port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[SSL Server] Failed to bind to port {}: {}", tls_port, e);
            return;
        }
    };

    println!("[SSL Server] Running HTTPS on https://localhost:{}", tls_port);

    loop {
        let (stream, _peer_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let acceptor = acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let alpn = tls_stream.get_ref().1.alpn_protocol().map(|p| p.to_vec());
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let service = hyper_util::service::TowerToHyperService::new(app);
                    
                    if alpn.as_deref() == Some(b"h2") {
                        if let Err(err) = hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                            .serve_connection(io, service)
                            .await 
                        {
                            eprintln!("[SSL Server] HTTP/2 connection error: {:?}", err);
                        }
                    } else {
                        if let Err(err) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, service)
                            .await 
                        {
                            eprintln!("[SSL Server] HTTP/1 connection error: {:?}", err);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[SSL Server] TLS handshake failed: {}", e);
                }
            }
        });
    }
}
