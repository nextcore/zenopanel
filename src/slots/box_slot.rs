use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write, BufRead, BufReader};
use std::process::Command;
use serde::{Serialize, Deserialize};
use serde_json::json;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

const DEFAULT_DATA_DIR: &str = "/var/lib/zeno-container";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerState {
    pub id: String,
    pub image: String,
    pub status: String, // created, running, stopped, failed
    pub pid: i32,
    pub created_at: String,
    pub exited_at: Option<String>,
    pub exit_code: Option<i32>,
    pub cmd: Vec<String>,
    pub log_path: Option<String>,
    pub ports: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub mounts: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub host_network: Option<bool>,
    pub restart_policy: Option<String>,
    pub desired_status: Option<String>,
    pub memory_limit: Option<i64>,
    pub cpu_limit: Option<f64>,
    pub oom_score_adj: Option<i32>,
    pub read_only: Option<bool>,
    pub network: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkConfig {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub subnet: String,
    pub gateway: String,
}

pub fn register(engine: &mut Engine) {
    register_box_pull(engine);
    register_box_images(engine);
    register_box_rmi(engine);
    register_box_create(engine);
    register_box_start(engine);
    register_box_stop(engine);
    register_box_delete(engine);
    register_box_list(engine);
    register_box_inspect(engine);
    register_box_logs(engine);
    register_box_rootfs_path(engine);
    register_box_update(engine);
    register_volume_list(engine);
    register_volume_create(engine);
    register_volume_delete(engine);
    register_network_list(engine);
    register_network_create(engine);
    register_network_delete(engine);
    register_box_compose(engine);
    register_box_compose_get_yaml(engine);
    register_box_compose_list_projects(engine);
    register_box_compose_delete_project(engine);
    register_box_registry_list(engine);
    register_box_registry_add(engine);
    register_box_registry_delete(engine);
    register_box_compose_git_get(engine);
    register_box_compose_git_save(engine);
    register_box_compose_git_sync(engine);
    register_box_prune(engine);
}

pub(crate) fn get_runc_bin() -> String {
    if let Ok(val) = std::env::var("ZENO_CONTAINER_RUNC") {
        return val;
    }
    if let Some(path) = look_path("runc") {
        return path.to_string_lossy().to_string();
    }
    
    // Fallback to embedded runc
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let dest_dir = Path::new(&home).join(".zeno-container/bin");
    let _ = fs::create_dir_all(&dest_dir);
    let dest_path = dest_dir.join("runc");
    if !dest_path.exists() {
        const RUNC_BYTES: &[u8] = include_bytes!("runc-linux-amd64");
        if let Ok(mut f) = File::create(&dest_path) {
            let _ = f.write_all(RUNC_BYTES);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = f.metadata().unwrap().permissions();
                perms.set_mode(0o755);
                let _ = f.set_permissions(perms);
            }
        }
    }
    dest_path.to_string_lossy().to_string()
}

fn look_path(name: &str) -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for path in std::env::split_paths(&path_var) {
            let full_path = path.join(name);
            if full_path.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(metadata) = full_path.metadata() {
                        if metadata.mode() & 0o111 != 0 {
                            return Some(full_path);
                        }
                    }
                }
                #[cfg(not(unix))]
                return Some(full_path);
            }
        }
    }
    #[cfg(unix)]
    {
        let common_dirs = [
            "/usr/sbin",
            "/usr/local/sbin",
            "/usr/bin",
            "/usr/local/bin",
            "/sbin",
            "/bin",
        ];
        for dir in common_dirs {
            let full_path = Path::new(dir).join(name);
            if full_path.is_file() {
                use std::os::unix::fs::MetadataExt;
                if let Ok(metadata) = full_path.metadata() {
                    if metadata.mode() & 0o111 != 0 {
                        return Some(full_path);
                    }
                }
            }
        }
    }
    None
}


fn runc_exec(args: &[&str]) -> io::Result<std::process::Output> {
    let runc_bin = get_runc_bin();
    let root = format!("{}/runc", get_data_dir());
    let mut all_args = vec!["--root", &root];
    all_args.extend_from_slice(args);
    Command::new(&runc_bin).args(&all_args).output()
}

struct ImageRef {
    registry: String,
    repository: String,
    tag: String,
}

fn parse_image_ref(image: &str) -> ImageRef {
    let mut registry = "https://registry-1.docker.io".to_string();
    let mut tag = "latest".to_string();
    let mut repo = image.to_string();

    let parts: Vec<&str> = image.splitn(2, '/').collect();
    if parts.len() == 2 && (parts[0].contains('.') || parts[0].contains(':')) {
        registry = format!("https://{}", parts[0]);
        repo = parts[1].to_string();
    }

    if let Some(idx) = repo.rfind(':') {
        tag = repo[idx+1..].to_string();
        repo = repo[..idx].to_string();
    }

    if registry == "https://registry-1.docker.io" && !repo.contains('/') {
        repo = format!("library/{}", repo);
    }

    ImageRef { registry, repository: repo, tag }
}

fn get_docker_auth_for_registry(registry_url: &str) -> Option<(String, String)> {
    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let paths: [PathBuf; 2] = [
        home.join(".docker/config.json"),
        PathBuf::from("/root/.docker/config.json"),
    ];

    let host = registry_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    for p in &paths {
        if p.exists() {
            if let Ok(content) = fs::read_to_string(p) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(auths) = config.get("auths").and_then(|a| a.as_object()) {
                        for (key, val) in auths {
                            let key_clean = key.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
                            let host_clean = host.trim_end_matches('/');
                            let is_match = key_clean == host_clean 
                                || (host_clean == "registry-1.docker.io" && key_clean == "index.docker.io/v1");
                            
                            if is_match {
                                if let Some(auth_str) = val.get("auth").and_then(|a| a.as_str()) {
                                    use base64::{Engine as _, engine::general_purpose::STANDARD};
                                    if let Ok(decoded) = STANDARD.decode(auth_str) {
                                        if let Ok(decoded_str) = String::from_utf8(decoded) {
                                            let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
                                            if parts.len() == 2 {
                                                return Some((parts[0].to_string(), parts[1].to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn parse_www_authenticate(header_val: &str) -> Option<(String, String, String)> {
    if !header_val.starts_with("Bearer ") {
        return None;
    }
    let params = &header_val[7..];
    let mut realm = None;
    let mut service = None;
    let mut scope = None;

    for part in params.split(',') {
        let kv: Vec<&str> = part.splitn(2, '=').collect();
        if kv.len() == 2 {
            let k = kv[0].trim();
            let v = kv[1].trim().trim_matches('"');
            if k == "realm" {
                realm = Some(v.to_string());
            } else if k == "service" {
                service = Some(v.to_string());
            } else if k == "scope" {
                scope = Some(v.to_string());
            }
        }
    }

    if let (Some(r), Some(s)) = (realm, service) {
        Some((r, s, scope.unwrap_or_default()))
    } else {
        None
    }
}

fn apply_auth_header(req: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    if token.is_empty() {
        req
    } else if token.starts_with("Bearer ") || token.starts_with("Basic ") {
        req.header("Authorization", token)
    } else {
        req.header("Authorization", format!("Bearer {}", token))
    }
}

async fn get_registry_token(client: &reqwest::Client, img: &ImageRef) -> Result<String, String> {
    let host = img.registry.trim_start_matches("https://").trim_start_matches("http://");
    let credentials = get_docker_auth_for_registry(&img.registry);

    if host == "registry-1.docker.io" {
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull",
            img.repository
        );
        let mut req = client.get(&auth_url);
        if let Some((ref username, ref password)) = credentials {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);
            req = req.header("Authorization", format!("Basic {}", encoded));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(tok) = json.get("token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
            if let Some(tok) = json.get("access_token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
        }
        
        if let Some((ref username, ref password)) = credentials {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);
            return Ok(format!("Basic {}", encoded));
        }
        
        return Ok(String::new());
    }

    let v2_url = format!("{}/v2/", img.registry);
    let mut req = client.get(&v2_url);
    if let Some((ref username, ref password)) = credentials {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let auth_val = format!("{}:{}", username, password);
        let encoded = STANDARD.encode(auth_val);
        req = req.header("Authorization", format!("Basic {}", encoded));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if resp.status() == 401 {
        if let Some(auth_header) = resp.headers().get("www-authenticate").and_then(|h| h.to_str().ok()) {
            if let Some((realm, service, scope)) = parse_www_authenticate(auth_header) {
                let final_scope = if scope.is_empty() {
                    format!("repository:{}:pull", img.repository)
                } else {
                    scope
                };
                let mut token_req = client.get(&realm)
                    .query(&[("service", &service), ("scope", &final_scope)]);
                
                if let Some((ref username, ref password)) = credentials {
                    use base64::{Engine as _, engine::general_purpose::STANDARD};
                    let auth_val = format!("{}:{}", username, password);
                    let encoded = STANDARD.encode(auth_val);
                    token_req = token_req.header("Authorization", format!("Basic {}", encoded));
                }
                
                let token_resp = token_req.send().await.map_err(|e| e.to_string())?;
                if token_resp.status().is_success() {
                    let json: serde_json::Value = token_resp.json().await.map_err(|e| e.to_string())?;
                    if let Some(tok) = json.get("token").and_then(|v| v.as_str()) {
                        return Ok(tok.to_string());
                    }
                    if let Some(tok) = json.get("access_token").and_then(|v| v.as_str()) {
                        return Ok(tok.to_string());
                    }
                }
            }
        }
    }

    if let Some((ref username, ref password)) = credentials {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let auth_val = format!("{}:{}", username, password);
        let encoded = STANDARD.encode(auth_val);
        return Ok(format!("Basic {}", encoded));
    }

    Ok(String::new())
}

async fn pull_image_rust(image: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let img_ref = parse_image_ref(image);
    let token = get_registry_token(&client, &img_ref).await?;

    let data_dir = get_data_dir();
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    
    let image_cache_dir = format!("{}/images/{}", data_dir, cache_dir_name);
    let layers_cache_dir = format!("{}/images/layers", data_dir);

    fs::create_dir_all(&image_cache_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&layers_cache_dir).map_err(|e| e.to_string())?;

    let manifest_url = format!("{}/v2/{}/manifests/{}", img_ref.registry, img_ref.repository, img_ref.tag);
    let mut req = client.get(&manifest_url)
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json");
    req = apply_auth_header(req, &token);
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch manifest: status {}", resp.status()));
    }

    let headers = resp.headers().clone();
    let content_type = headers.get("Content-Type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let body_bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let mut manifest_json: serde_json::Value = serde_json::from_slice(&body_bytes).map_err(|e| e.to_string())?;

    if content_type.contains("manifest.list") || content_type.contains("image.index") {
        let mut selected_digest = None;
        if let Some(manifests) = manifest_json.get("manifests").and_then(|m| m.as_array()) {
            for m in manifests {
                let platform = m.get("platform");
                let os = platform.and_then(|p| p.get("os")).and_then(|o| o.as_str()).unwrap_or("");
                let arch = platform.and_then(|p| p.get("architecture")).and_then(|a| a.as_str()).unwrap_or("");
                if os == "linux" && arch == "amd64" {
                    selected_digest = m.get("digest").and_then(|d| d.as_str()).map(|s| s.to_string());
                    break;
                }
            }
            if selected_digest.is_none() && !manifests.is_empty() {
                selected_digest = manifests[0].get("digest").and_then(|d| d.as_str()).map(|s| s.to_string());
            }
        }

        let digest = selected_digest.ok_or_else(|| "No matching manifest in list".to_string())?;
        
        let manifest_by_digest_url = format!("{}/v2/{}/manifests/{}", img_ref.registry, img_ref.repository, digest);
        let mut req2 = client.get(&manifest_by_digest_url).header("Accept", "application/vnd.docker.distribution.manifest.v2+json");
        req2 = apply_auth_header(req2, &token);
        let resp2 = req2.send().await.map_err(|e| e.to_string())?;
        if !resp2.status().is_success() {
            return Err(format!("Failed to fetch resolved manifest: status {}", resp2.status()));
        }
        let body2 = resp2.bytes().await.map_err(|e| e.to_string())?;
        manifest_json = serde_json::from_slice(&body2).map_err(|e| e.to_string())?;
    }

    let config_digest = manifest_json.get("config")
        .and_then(|c| c.get("digest"))
        .and_then(|d| d.as_str())
        .ok_or_else(|| "Missing config digest in manifest".to_string())?;

    let config_url = format!("{}/v2/{}/blobs/{}", img_ref.registry, img_ref.repository, config_digest);
    let mut req_cfg = client.get(&config_url);
    req_cfg = apply_auth_header(req_cfg, &token);
    let resp_cfg = req_cfg.send().await.map_err(|e| e.to_string())?;
    let config_bytes = resp_cfg.bytes().await.map_err(|e| e.to_string())?;
    let image_config_json: serde_json::Value = serde_json::from_slice(&config_bytes).map_err(|e| e.to_string())?;

    let layers = manifest_json.get("layers")
        .and_then(|l| l.as_array())
        .ok_or_else(|| "Missing layers in manifest".to_string())?;

    let mut layer_digests = Vec::new();
    for (_i, layer) in layers.iter().enumerate() {
        let digest = layer.get("digest").and_then(|d| d.as_str()).ok_or_else(|| "Missing layer digest".to_string())?;
        let digest_clean = digest.trim_start_matches("sha256:");
        layer_digests.push(digest_clean.to_string());

        let layer_dir = format!("{}/{}", layers_cache_dir, digest_clean);
        let layer_rootfs = format!("{}/rootfs", layer_dir);
        let tar_gz_path = format!("{}/{}.tar.gz", layer_dir, digest_clean);

        fs::create_dir_all(&layer_dir).map_err(|e| e.to_string())?;

        if !Path::new(&layer_rootfs).exists() {
            if !Path::new(&tar_gz_path).exists() {
                let blob_url = format!("{}/v2/{}/blobs/{}", img_ref.registry, img_ref.repository, digest);
                let mut req_blob = client.get(&blob_url);
                req_blob = apply_auth_header(req_blob, &token);
                let resp_blob = req_blob.send().await.map_err(|e| e.to_string())?;
                if !resp_blob.status().is_success() {
                    return Err(format!("Layer download failed: status {}", resp_blob.status()));
                }
                let blob_bytes = resp_blob.bytes().await.map_err(|e| e.to_string())?;
                fs::write(&tar_gz_path, &blob_bytes).map_err(|e| e.to_string())?;
            }

            fs::create_dir_all(&layer_rootfs).map_err(|e| e.to_string())?;
            let tar_gz = File::open(&tar_gz_path).map_err(|e| e.to_string())?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&layer_rootfs).map_err(|e| e.to_string())?;
            
            // Delete compressed .tar.gz archive to prevent double storage disk usage
            let _ = fs::remove_file(&tar_gz_path);
        }
    }

    fs::write(
        format!("{}/layers.json", image_cache_dir),
        serde_json::to_string(&layer_digests).unwrap()
    ).map_err(|e| e.to_string())?;

    fs::write(
        format!("{}/image-config.json", image_cache_dir),
        serde_json::to_string_pretty(&image_config_json).unwrap()
    ).map_err(|e| e.to_string())?;

    let mut final_cmd = Vec::new();
    if let Some(entrypoint) = image_config_json.get("config").and_then(|c| c.get("Entrypoint")).and_then(|e| e.as_array()) {
        for val in entrypoint {
            if let Some(s) = val.as_str() {
                final_cmd.push(s.to_string());
            }
        }
    }
    if let Some(cmd) = image_config_json.get("config").and_then(|c| c.get("Cmd")).and_then(|e| e.as_array()) {
        for val in cmd {
            if let Some(s) = val.as_str() {
                final_cmd.push(s.to_string());
            }
        }
    }

    // Resolve relative script names like 'docker-entrypoint.sh' to absolute paths to prevent OCI execution errors
    if !final_cmd.is_empty() && final_cmd[0] == "docker-entrypoint.sh" {
        final_cmd[0] = "/usr/local/bin/docker-entrypoint.sh".to_string();
    }

    Ok(final_cmd)
}

fn get_image_default_cmd(image: &str) -> Vec<String> {
    let data_dir = get_data_dir();
    let img_ref = parse_image_ref(image);
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    let cache_dir = Path::new(&data_dir).join("images").join(&cache_dir_name);
    let config_path = cache_dir.join("image-config.json");
    let mut default_cmd = Vec::new();
    if config_path.exists() {
        if let Ok(file) = File::open(&config_path) {
            if let Ok(image_config_json) = serde_json::from_reader::<_, serde_json::Value>(file) {
                if let Some(entrypoint) = image_config_json.get("config").and_then(|c| c.get("Entrypoint")).and_then(|e| e.as_array()) {
                    for val in entrypoint {
                        if let Some(s) = val.as_str() {
                            default_cmd.push(s.to_string());
                        }
                    }
                }
                if let Some(cmd) = image_config_json.get("config").and_then(|c| c.get("Cmd")).and_then(|e| e.as_array()) {
                    for val in cmd {
                        if let Some(s) = val.as_str() {
                            default_cmd.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    if !default_cmd.is_empty() && default_cmd[0] == "docker-entrypoint.sh" {
        default_cmd[0] = "/usr/local/bin/docker-entrypoint.sh".to_string();
    }

    default_cmd
}

fn get_data_dir() -> String {
    if let Ok(dir) = std::env::var("ZENO_CONTAINER_DATA_DIR") {
        return dir;
    }

    let default_path = std::path::Path::new(DEFAULT_DATA_DIR);
    let mut is_writable = false;
    if default_path.exists() {
        let test_file = default_path.join(format!(".test_write_{}", rand::random::<u32>()));
        if std::fs::write(&test_file, "test").is_ok() {
            is_writable = true;
            let _ = std::fs::remove_file(test_file);
        }
    } else {
        if std::fs::create_dir_all(default_path).is_ok() {
            is_writable = true;
        }
    }

    if is_writable {
        DEFAULT_DATA_DIR.to_string()
    } else {
        let fallback_path = if let Ok(home) = std::env::var("HOME") {
            format!("{}/.zeno-container", home)
        } else {
            "./data/zeno-container".to_string()
        };

        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("⚠ [BoxSlot] Default directory {} is not writable. Falling back to local directory: {}", DEFAULT_DATA_DIR, fallback_path);
            LOGGED.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        let _ = std::fs::create_dir_all(&fallback_path);
        fallback_path
    }
}

fn container_dir(data_dir: &str, id: &str) -> PathBuf {
    Path::new(data_dir).join("containers").join(id)
}

fn bundle_dir(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("bundle")
}

fn rootfs_dir(data_dir: &str, id: &str) -> PathBuf {
    bundle_dir(data_dir, id).join("rootfs")
}

fn state_file(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("state.json")
}

fn log_path(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("console.log")
}

fn is_overlay_mounted(mount_point: &str) -> bool {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    mounts.lines().any(|line| {
        let parts: Vec<&str> = line.split_whitespace().collect();
        parts.len() >= 3 && parts[2] == "overlay" && parts[1] == mount_point
    })
}

fn run_privileged_status(cmd: &str, args: &[&str]) -> io::Result<std::process::ExitStatus> {
    let is_root = unsafe { libc::getuid() == 0 };
    if is_root {
        Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    } else {
        let mut all_args = vec![cmd];
        all_args.extend_from_slice(args);
        Command::new("sudo")
            .args(&all_args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    }
}

fn run_cmd_status_silent(cmd: &str, args: &[&str]) -> io::Result<std::process::ExitStatus> {
    Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
}

fn mount_overlayfs(image: &str, data_dir: &str, id: &str) -> Result<(), String> {
    let dst_rootfs = rootfs_dir(data_dir, id);

    // Check /proc/mounts to verify the overlay is actually mounted (not just that
    // the directory exists — dst_rootfs is created before mount, so existence alone
    // does not mean the mount succeeded on a prior attempt).
    if is_overlay_mounted(&dst_rootfs.to_string_lossy()) {
        return Ok(());
    }

    let img_ref = parse_image_ref(image);
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    let image_cache_dir = Path::new(data_dir).join("images").join(&cache_dir_name);

    let layers_json_path = image_cache_dir.join("layers.json");
    if !layers_json_path.exists() {
        return Err(format!("Image metadata layers.json not found for {}", image));
    }

    let file = File::open(&layers_json_path).map_err(|e| e.to_string())?;
    let layers: Vec<String> = serde_json::from_reader(file).map_err(|e| e.to_string())?;

    let mut lowerdirs = Vec::new();
    let layers_dir = Path::new(data_dir).join("images").join("layers");
    for layer in layers.iter().rev() {
        lowerdirs.push(layers_dir.join(layer).join("rootfs").to_string_lossy().to_string());
    }
    let lowerdir_str = lowerdirs.join(":");

    let cont_dir = container_dir(data_dir, id);
    let upperdir = cont_dir.join("diff");
    let workdir = cont_dir.join("work");

    fs::create_dir_all(&upperdir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&workdir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dst_rootfs).map_err(|e| e.to_string())?;

    let opts = format!("lowerdir={},upperdir={},workdir={}", lowerdir_str, upperdir.to_string_lossy(), workdir.to_string_lossy());
    let dst_rootfs_str = dst_rootfs.to_string_lossy().to_string();

    // Use sudo only if not already root, so the mount command succeeds regardless of whether the zeno
    // process itself was started with root privileges.
    let status = run_privileged_status("mount", &["-t", "overlay", "overlay", "-o", &opts, &dst_rootfs_str])
        .map_err(|e| format!("Failed to run mount command: {}", e))?;

    if !status.success() {
        // Fallback: Mirror Docker's VFS driver by copying layer files recursively
        // when overlayfs mount is unsupported (e.g. on OpenVZ / Virtuozzo VPS)
        for layer in &layers {
            let src_rootfs = layers_dir.join(layer).join("rootfs");
            if src_rootfs.exists() {
                let src_str = format!("{}/.", src_rootfs.to_string_lossy());
                let dst_str = dst_rootfs.to_string_lossy().to_string();
                let cp_status = run_privileged_status("cp", &["-a", &src_str, &dst_str])
                    .map_err(|e| format!("Failed to run cp command for VFS fallback: {}", e))?;
                if !cp_status.success() {
                    return Err(format!(
                        "Overlay mount failed, and VFS copy fallback failed for layer: {}",
                        layer
                    ));
                }
            }
        }
        // Process OCI whiteouts after copying all layers to correctly handle deleted files
        let _ = process_whiteouts(&dst_rootfs);
    }

    Ok(())
}

fn process_whiteouts(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(".wh.") {
                    if file_name == ".wh..wh..opq" {
                        let _ = fs::remove_file(&path);
                    } else {
                        let target_name = file_name.trim_start_matches(".wh.");
                        let target_path = dir.join(target_name);
                        if let Ok(meta) = target_path.symlink_metadata() {
                            if meta.is_dir() {
                                let _ = fs::remove_dir_all(&target_path);
                            } else {
                                let _ = fs::remove_file(&target_path);
                            }
                        }
                        let _ = fs::remove_file(&path);
                    }
                } else if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() && !file_type.is_symlink() {
                        let _ = process_whiteouts(&path);
                    }
                }
            }
        }
    }
    Ok(())
}

fn generate_config_json(
    bundle_dir: &Path,
    cmd: Vec<String>,
    env: HashMap<String, String>,
    cwd: &str,
    mounts: Vec<String>,
    host_network: bool,
    memory_limit: i64,
    cpu_limit: f64,
    oom_score_adj: Option<i32>,
    read_only: bool,
) -> Result<(), String> {
    let is_rootless = unsafe { libc::getuid() != 0 };

    let mut process_env = vec![
        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        "TERM=xterm".to_string(),
        "HOME=/root".to_string(),
    ];
    for (k, v) in env {
        process_env.push(format!("{}={}", k, v));
    }

    let mut oci_mounts = vec![
        json!({
            "destination": "/proc",
            "type": "proc",
            "source": "proc"
        }),
        json!({
            "destination": "/dev",
            "type": "tmpfs",
            "source": "tmpfs",
            "options": ["nosuid", "strictatime", "mode=755", "size=65536k"]
        })
    ];

    if read_only {
        oci_mounts.push(json!({
            "destination": "/tmp",
            "type": "tmpfs",
            "source": "tmpfs",
            "options": ["nosuid", "nodev", "mode=1777", "size=65536k"]
        }));
        oci_mounts.push(json!({
            "destination": "/run",
            "type": "tmpfs",
            "source": "tmpfs",
            "options": ["nosuid", "nodev", "mode=755", "size=65536k"]
        }));
    }

    if !is_rootless {
        oci_mounts.push(json!({
            "destination": "/dev/pts",
            "type": "devpts",
            "source": "devpts",
            "options": ["nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620"]
        }));
        oci_mounts.push(json!({
            "destination": "/dev/shm",
            "type": "tmpfs",
            "source": "shm",
            "options": ["nosuid", "noexec", "nodev", "mode=1777", "size=65536k"]
        }));
        oci_mounts.push(json!({
            "destination": "/sys",
            "type": "sysfs",
            "source": "sysfs",
            "options": ["nosuid", "noexec", "nodev", "ro"]
        }));
    }

    let data_dir = get_data_dir();
    for m in mounts {
        let parts: Vec<&str> = m.splitn(2, ':').collect();
        if parts.len() == 2 {
            let host_path = parts[0];
            let container_path = parts[1];

            let is_named_volume = !host_path.starts_with('/') && !host_path.starts_with('.') && !host_path.starts_with('~');
            let resolved_host_path = if is_named_volume {
                Path::new(&data_dir).join("volumes").join(host_path)
            } else {
                PathBuf::from(host_path)
            };

            // Auto-create host path directory if it doesn't exist
            if !resolved_host_path.exists() {
                let _ = fs::create_dir_all(&resolved_host_path);
            }

            let abs_host = fs::canonicalize(&resolved_host_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| resolved_host_path.to_string_lossy().to_string());

            oci_mounts.push(json!({
                "destination": container_path,
                "type": "bind",
                "source": abs_host,
                "options": ["bind", "rprivate", "rw"]
            }));
        }
    }

    let mut namespaces = vec![
        json!({ "type": "pid" }),
        json!({ "type": "ipc" }),
        json!({ "type": "uts" }),
        json!({ "type": "mount" })
    ];
    if is_rootless {
        namespaces.push(json!({ "type": "user" }));
    }
    if !host_network {
        namespaces.push(json!({ "type": "network" }));
    }

    let mut resources = json!({});
    if memory_limit > 0 {
        resources["memory"] = json!({ "limit": memory_limit });
    }
    if cpu_limit > 0.0 {
        let period = 100000u64;
        let quota = (cpu_limit * 100000.0) as i64;
        resources["cpu"] = json!({
            "period": period,
            "quota": quota
        });
    }

    let spec = json!({
        "ociVersion": "1.0.2",
        "process": {
            "terminal": false,
            "user": {
                "uid": 0,
                "gid": 0
            },
            "args": cmd,
            "env": process_env,
            "cwd": if cwd.is_empty() { "/" } else { cwd },
            "oomScoreAdj": oom_score_adj,
            "rlimits": [
                {
                    "type": "RLIMIT_NOFILE",
                    "hard": 65536,
                    "soft": 65536
                }
            ],
            "capabilities": if is_rootless { serde_json::Value::Null } else {
                json!({
                    "bounding": [
                        "CAP_AUDIT_WRITE",
                        "CAP_CHOWN",
                        "CAP_DAC_OVERRIDE",
                        "CAP_FOWNER",
                        "CAP_FSETID",
                        "CAP_KILL",
                        "CAP_MKNOD",
                        "CAP_NET_BIND_SERVICE",
                        "CAP_NET_RAW",
                        "CAP_SETGID",
                        "CAP_SETFCAP",
                        "CAP_SETUID",
                        "CAP_SETPCAP",
                        "CAP_SYS_CHROOT"
                    ],
                    "effective": [
                        "CAP_AUDIT_WRITE",
                        "CAP_CHOWN",
                        "CAP_DAC_OVERRIDE",
                        "CAP_FOWNER",
                        "CAP_FSETID",
                        "CAP_KILL",
                        "CAP_MKNOD",
                        "CAP_NET_BIND_SERVICE",
                        "CAP_NET_RAW",
                        "CAP_SETGID",
                        "CAP_SETFCAP",
                        "CAP_SETUID",
                        "CAP_SETPCAP",
                        "CAP_SYS_CHROOT"
                    ],
                    "inheritable": [
                        "CAP_AUDIT_WRITE",
                        "CAP_CHOWN",
                        "CAP_DAC_OVERRIDE",
                        "CAP_FOWNER",
                        "CAP_FSETID",
                        "CAP_KILL",
                        "CAP_MKNOD",
                        "CAP_NET_BIND_SERVICE",
                        "CAP_NET_RAW",
                        "CAP_SETGID",
                        "CAP_SETFCAP",
                        "CAP_SETUID",
                        "CAP_SETPCAP",
                        "CAP_SYS_CHROOT"
                    ],
                    "permitted": [
                        "CAP_AUDIT_WRITE",
                        "CAP_CHOWN",
                        "CAP_DAC_OVERRIDE",
                        "CAP_FOWNER",
                        "CAP_FSETID",
                        "CAP_KILL",
                        "CAP_MKNOD",
                        "CAP_NET_BIND_SERVICE",
                        "CAP_NET_RAW",
                        "CAP_SETGID",
                        "CAP_SETFCAP",
                        "CAP_SETUID",
                        "CAP_SETPCAP",
                        "CAP_SYS_CHROOT"
                    ]
                })
            }
        },
        "root": {
            "path": "rootfs",
            "readonly": read_only
        },
        "hostname": "zeno-box",
        "mounts": oci_mounts,
        "linux": {
            "resources": resources,
            "namespaces": namespaces,
            "uidMappings": if is_rootless {
                Some(json!([{ "containerID": 0, "hostID": unsafe { libc::getuid() }, "size": 1 }]))
            } else { None },
            "gidMappings": if is_rootless {
                Some(json!([{ "containerID": 0, "hostID": unsafe { libc::getgid() }, "size": 1 }]))
            } else { None },
            "maskedPaths": if is_rootless { serde_json::Value::Null } else {
                json!([
                    "/proc/acpi", "/proc/asound", "/proc/kcore", "/proc/keys",
                    "/proc/latency_stats", "/proc/timer_list", "/proc/timer_stats",
                    "/proc/sched_debug", "/sys/firmware"
                ])
            },
            "readonlyPaths": if is_rootless { serde_json::Value::Null } else {
                json!([
                    "/proc/bus", "/proc/fs", "/proc/irq", "/proc/sys", "/proc/sysrq-trigger"
                ])
            }
        }
    });

    let config_path = bundle_dir.join("config.json");
    let file = File::create(config_path).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file, &spec).map_err(|e| e.to_string())?;

    Ok(())
}

fn container_create(
    id: &str,
    image: &str,
    cmd: Vec<String>,
    env: HashMap<String, String>,
    cwd: &str,
    mounts: Vec<String>,
    ports: Vec<String>,
    host_network: bool,
    restart_policy: &str,
    memory_limit: i64,
    cpu_limit: f64,
    oom_score_adj: Option<i32>,
    read_only: bool,
    network: &str,
) -> Result<(), String> {
    let data_dir = get_data_dir();
    let state_p = state_file(&data_dir, id);
    if state_p.exists() {
        return Err(format!("Container {} already exists", id));
    }

    let bundle_p = bundle_dir(&data_dir, id);
    fs::create_dir_all(&bundle_p).map_err(|e| e.to_string())?;

    mount_overlayfs(image, &data_dir, id)?;

    let mut resolved_cwd = cwd.to_string();
    let mut merged_env = env.clone();

    let img_ref = parse_image_ref(image);
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    let image_config_path = Path::new(&data_dir)
        .join("images")
        .join(&cache_dir_name)
        .join("image-config.json");
    if image_config_path.exists() {
        if let Ok(file) = File::open(&image_config_path) {
            if let Ok(cfg) = serde_json::from_reader::<_, serde_json::Value>(file) {
                if resolved_cwd.is_empty() {
                    if let Some(workdir) = cfg.get("config")
                        .and_then(|c| c.get("WorkingDir"))
                        .and_then(|w| w.as_str()) 
                    {
                        resolved_cwd = workdir.to_string();
                    }
                }

                if let Some(env_array) = cfg.get("config")
                    .and_then(|c| c.get("Env"))
                    .and_then(|e| e.as_array())
                {
                    for item in env_array {
                        if let Some(env_str) = item.as_str() {
                            let parts: Vec<&str> = env_str.splitn(2, '=').collect();
                            if parts.len() == 2 {
                                let k = parts[0].to_string();
                                let v = parts[1].to_string();
                                merged_env.entry(k).or_insert(v);
                            }
                        }
                    }
                }
            }
        }
    }

    // Workaround for Bitnami image shadow-reinstall bug:
    // If 'bitnami' user exists in /etc/passwd but 'bitnami' group is missing in /etc/group,
    // inject the 'bitnami' group into /etc/group so that chown commands succeed.
    let container_group_path = bundle_p.join("rootfs").join("etc").join("group");
    let container_passwd_path = bundle_p.join("rootfs").join("etc").join("passwd");
    if container_passwd_path.exists() && container_group_path.exists() {
        if let Ok(passwd_content) = fs::read_to_string(&container_passwd_path) {
            if passwd_content.contains("bitnami:") {
                if let Ok(group_content) = fs::read_to_string(&container_group_path) {
                    if !group_content.contains("bitnami:") {
                        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&container_group_path) {
                            let _ = writeln!(file, "bitnami:x:1000:");
                        }
                    }
                }
            }
        }
    }

    generate_config_json(
        &bundle_p,
        cmd.clone(),
        merged_env.clone(),
        &resolved_cwd,
        mounts.clone(),
        host_network,
        memory_limit,
        cpu_limit,
        oom_score_adj,
        read_only,
    )?;

    let c_log_path = log_path(&data_dir, id).to_string_lossy().to_string();
    let state = ContainerState {
        id: id.to_string(),
        image: image.to_string(),
        status: "created".to_string(),
        pid: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
        exited_at: None,
        exit_code: None,
        cmd,
        log_path: Some(c_log_path),
        ports: Some(ports),
        env: Some(merged_env),
        mounts: Some(mounts),
        cwd: Some(resolved_cwd),
        host_network: Some(host_network),
        restart_policy: Some(restart_policy.to_string()),
        desired_status: Some("stopped".to_string()),
        memory_limit: Some(memory_limit),
        cpu_limit: Some(cpu_limit),
        oom_score_adj,
        read_only: Some(read_only),
        network: Some(network.to_string()),
    };

    save_container_state(&state)?;

    Ok(())
}

fn save_container_state(state: &ContainerState) -> Result<(), String> {
    let data_dir = get_data_dir();
    let p = state_file(&data_dir, &state.id);
    let f = File::create(p).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(f, state).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_container_state(id: &str) -> Result<ContainerState, String> {
    let data_dir = get_data_dir();
    let p = state_file(&data_dir, id);
    let f = File::open(p).map_err(|e| e.to_string())?;
    let state: ContainerState = serde_json::from_reader(f).map_err(|e| e.to_string())?;
    Ok(state)
}

fn get_networks(data_dir: &str) -> Vec<NetworkConfig> {
    let path = Path::new(data_dir).join("networks.json");
    if !path.exists() {
        return Vec::new();
    }
    if let Ok(f) = File::open(path) {
        if let Ok(nets) = serde_json::from_reader(f) {
            return nets;
        }
    }
    Vec::new()
}

fn save_networks(data_dir: &str, nets: &[NetworkConfig]) -> Result<(), String> {
    let path = Path::new(data_dir).join("networks.json");
    let f = File::create(path).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(f, nets).map_err(|e| e.to_string())?;
    Ok(())
}

fn setup_bridge() -> Result<(), String> {
    // Enable IP forwarding — required for container port mapping via iptables DNAT to work.
    // This must run every boot (sysctl settings are not persistent across reboots).
    let _ = std::fs::write("/proc/sys/net/ipv4/ip_forward", "1");

    let bridge_exists = Command::new("ip").args(&["link", "show", "zenobr0"]).output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !bridge_exists {
        let _ = run_cmd_status_silent("ip", &["link", "add", "name", "zenobr0", "type", "bridge"]);
        let _ = run_cmd_status_silent("ip", &["addr", "add", "172.20.0.1/16", "dev", "zenobr0"]);
        let _ = run_cmd_status_silent("ip", &["link", "set", "zenobr0", "up"]);
    }

    // Enable route_localnet on zenobr0 bridge to allow loopback NAT MASQUERADE.
    // Without this, the Linux kernel treats loopback-sourced NAT packets as Martians and drops them.
    let _ = std::fs::write("/proc/sys/net/ipv4/conf/zenobr0/route_localnet", "1");

    // Always ensure POSTROUTING MASQUERADE rule exists (needed for container internet access).
    // Check first to prevent duplicate entries.
    let masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"]);
    }

    // Always ensure FORWARD rules exist for zenobr0 so that DNAT port-mapped traffic
    // can actually reach the container. Without this, packets are dropped by the
    // FORWARD chain even though DNAT re-routes them correctly.
    let fwd_in_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-i", "zenobr0", "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_in_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-i", "zenobr0", "-j", "ACCEPT"]);
    }

    let fwd_out_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-o", "zenobr0", "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_out_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-o", "zenobr0", "-j", "ACCEPT"]);
    }

    // Always ensure loopback requests to port-forwarded ports on zenobr0 are masqueraded.
    // This translates the source IP from 127.0.0.1 to the bridge gateway IP (172.20.0.1)
    // so the packet doesn't get dropped by kernel Martian filters and can route back to host.
    let local_masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-o", "zenobr0", "-s", "127.0.0.1", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !local_masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-o", "zenobr0", "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    }

    // Always ensure CHECKSUM fill rule exists for zenobr0.
    // This resolves the bad TCP checksum drop issue when accessing container ports from localhost.
    let chk_exists = run_cmd_status_silent("iptables", &["-t", "mangle", "-C", "POSTROUTING", "-o", "zenobr0", "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !chk_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-A", "POSTROUTING", "-o", "zenobr0", "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    }

    Ok(())
}

fn find_available_ip(data_dir: &str, subnet: &str, gateway: &str) -> Result<String, String> {
    let parts: Vec<&str> = subnet.split('.').collect();
    if parts.len() < 2 {
        return Err(format!("Invalid subnet: {}", subnet));
    }
    let x: i32 = parts[1].parse().map_err(|e| format!("Invalid subnet number: {}", e))?;

    let mut taken_ips = std::collections::HashSet::new();
    taken_ips.insert(gateway.to_string());

    if let Ok(containers) = container_list_internal(data_dir, false) {
        for c in containers {
            if c.status == "running" {
                if let Some(env) = c.env {
                    if let Some(ip) = env.get("ZENO_IP") {
                        taken_ips.insert(ip.clone());
                    }
                }
            }
        }
    }

    for i in 2..255 {
        let ip = format!("172.{}.0.{}", x, i);
        if !taken_ips.contains(&ip) {
            return Ok(ip);
        }
    }

    Err("No available IP addresses".to_string())
}

unsafe extern "C" {
    fn ioctl(
        fd: std::os::raw::c_int,
        request: std::os::raw::c_ulong,
        ...
    ) -> std::os::raw::c_int;
}

fn disable_checksum_offloading(iface: &str) -> Result<(), String> {
    use std::os::unix::io::AsRawFd;

    #[repr(C)]
    struct EthtoolValue {
        cmd: u32,
        data: u32,
    }

    #[repr(C)]
    struct IfreqEthtool {
        ifr_name: [u8; 16],
        ifr_data: *mut EthtoolValue,
    }

    const SIOCETHTOOL: std::os::raw::c_ulong = 0x8946;
    const ETHTOOL_SRXCSUM: u32 = 0x00000015;
    const ETHTOOL_STXCSUM: u32 = 0x00000017;

    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to create socket: {}", e))?;
    let fd = socket.as_raw_fd();

    let mut ifr_name = [0u8; 16];
    let bytes = iface.as_bytes();
    if bytes.len() >= 16 {
        return Err("Interface name too long".to_string());
    }
    ifr_name[..bytes.len()].copy_from_slice(bytes);

    // Disable RX checksumming
    let mut rx_val = EthtoolValue {
        cmd: ETHTOOL_SRXCSUM,
        data: 0,
    };
    let mut ifr_rx = IfreqEthtool {
        ifr_name,
        ifr_data: &mut rx_val,
    };
    unsafe {
        let _ = ioctl(fd, SIOCETHTOOL, &mut ifr_rx);
    }

    // Disable TX checksumming
    let mut tx_val = EthtoolValue {
        cmd: ETHTOOL_STXCSUM,
        data: 0,
    };
    let mut ifr_tx = IfreqEthtool {
        ifr_name,
        ifr_data: &mut tx_val,
    };
    unsafe {
        let _ = ioctl(fd, SIOCETHTOOL, &mut ifr_tx);
    }

    Ok(())
}

fn configure_container_network(
    data_dir: &str,
    container_id: &str,
    pid: i32,
    ports: Vec<String>,
    network_name: &str,
) -> Result<String, String> {
    let mut bridge_id = "zenobr0".to_string();
    let mut subnet_str = "172.20.0.0/16".to_string();
    let mut gateway_ip = "172.20.0.1".to_string();

    if !network_name.is_empty() && network_name != "bridge" && network_name != "default" {
        let networks = get_networks(data_dir);
        for n in networks {
            if n.name == network_name || n.id == network_name {
                bridge_id = n.id;
                subnet_str = n.subnet;
                gateway_ip = n.gateway;
                break;
            }
        }
    }

    if bridge_id == "zenobr0" {
        setup_bridge()?;
    } else {
        let output = Command::new("ip").args(&["link", "show", &bridge_id]).output();
        if output.is_err() || !output.unwrap().status.success() {
            return Err(format!("Custom bridge interface {} does not exist", bridge_id));
        }
    }

    let ip = find_available_ip(data_dir, &subnet_str, &gateway_ip)?;

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash_val = hasher.finish();
    let hash_str = format!("{:08x}", hash_val);
    let short_hash = if hash_str.len() > 8 { &hash_str[0..8] } else { &hash_str };

    let veth_host = format!("veth-h-{}", short_hash);
    let veth_guest = format!("veth-g-{}", short_hash);

    let _ = run_cmd_status_silent("ip", &["link", "delete", &veth_host]);

    let status = run_cmd_status_silent("ip", &["link", "add", &veth_host, "type", "veth", "peer", "name", &veth_guest])
        .map_err(|e| format!("Failed to create veth pair: {}", e))?;
    if !status.success() {
        return Err("Failed to create veth pair".to_string());
    }

    // Disable TX and RX checksum offloading on both host and guest veth interfaces
    // to prevent bad/blank TCP checksums from dropping packets inside the container.
    // We do this directly via raw ioctls in Rust, completely removing the dependency on external ethtool bin.
    let _ = disable_checksum_offloading(&veth_host);
    let _ = disable_checksum_offloading(&veth_guest);

    let status = run_cmd_status_silent("ip", &["link", "set", &veth_host, "master", &bridge_id])
        .map_err(|e| format!("Failed to bind host interface to bridge: {}", e))?;
    if !status.success() {
        return Err(format!("Failed to bind host interface {} to bridge {}", veth_host, bridge_id));
    }

    let status = run_cmd_status_silent("ip", &["link", "set", &veth_host, "up"])
        .map_err(|e| format!("Failed to bring up host interface: {}", e))?;
    if !status.success() {
        return Err(format!("Failed to bring up host interface {}", veth_host));
    }

    let pid_str = pid.to_string();
    let status = run_cmd_status_silent("ip", &["link", "set", &veth_guest, "netns", &pid_str])
        .map_err(|e| format!("Failed to move guest veth: {}", e))?;
    if !status.success() {
        return Err("Failed to move guest interface to container netns".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "link", "set", &veth_guest, "name", "eth0"])
        .map_err(|e| format!("Failed to rename veth inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to rename guest interface to eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "addr", "add", &format!("{}/16", ip), "dev", "eth0"])
        .map_err(|e| format!("Failed to configure IP inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to assign IP address to eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "link", "set", "eth0", "up"])
        .map_err(|e| format!("Failed to bring up link inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to bring up eth0 inside container".to_string());
    }

    let status = run_cmd_status_silent("nsenter", &["-t", &pid_str, "-n", "ip", "route", "add", "default", "via", &gateway_ip])
        .map_err(|e| format!("Failed to add gateway route inside namespace: {}", e))?;
    if !status.success() {
        return Err("Failed to configure default gateway route inside container".to_string());
    }

    let resolv_path = rootfs_dir(data_dir, container_id).join("etc/resolv.conf");
    let _ = fs::write(resolv_path, "nameserver 8.8.8.8\nnameserver 1.1.1.1\n");

    for p in ports {
        if let Some(rule) = parse_port_rule(&p) {
            let host_port_formatted = rule.host_port.replace('-', ":");
            let dest_str = format!("{}:{}", ip, rule.container_port);
            let mut preroute_args = vec!["-t", "nat", "-A", "PREROUTING", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                preroute_args.push("-d");
                preroute_args.push(hip);
            }
            preroute_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &preroute_args);

            let mut output_args = vec!["-t", "nat", "-A", "OUTPUT", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                output_args.push("-d");
                output_args.push(hip);
            }
            output_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &output_args);
        }
    }

    Ok(ip)
}

fn clean_container_network(container_id: &str, ip: &str, ports: &[String]) {
    for p in ports {
        if let Some(rule) = parse_port_rule(&p) {
            let host_port_formatted = rule.host_port.replace('-', ":");
            let dest_str = format!("{}:{}", ip, rule.container_port);
            let mut preroute_args = vec!["-t", "nat", "-D", "PREROUTING", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                preroute_args.push("-d");
                preroute_args.push(hip);
            }
            preroute_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &preroute_args);

            let mut output_args = vec!["-t", "nat", "-D", "OUTPUT", "-p", &rule.protocol];
            if let Some(ref hip) = rule.host_ip {
                output_args.push("-d");
                output_args.push(hip);
            }
            output_args.extend(&["--dport", &host_port_formatted, "-j", "DNAT", "--to-destination", &dest_str]);
            let _ = run_cmd_status_silent("iptables", &output_args);
        }
    }
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash_val = hasher.finish();
    let hash_str = format!("{:08x}", hash_val);
    let short_hash = if hash_str.len() > 8 { &hash_str[0..8] } else { &hash_str };

    let veth_host = format!("veth-h-{}", short_hash);
    let _ = run_cmd_status_silent("ip", &["link", "delete", &veth_host]);
}

fn sync_hosts_entries(data_dir: &str) -> Result<(), String> {
    let containers = container_list_internal(data_dir, false)?;
    
    let mut running_ips = HashMap::new();
    let mut running_nets = HashMap::new();
    for c in &containers {
        if c.status == "running" {
            if let Some(ref env) = c.env {
                if let Some(ip) = env.get("ZENO_IP") {
                    running_ips.insert(c.id.clone(), ip.clone());
                    if let Some(ref net) = c.network {
                        running_nets.insert(c.id.clone(), net.clone());
                    }
                }
            }
        }
    }

    for c in &containers {
        if c.status != "running" {
            continue;
        }

        let hosts_path = rootfs_dir(data_dir, &c.id).join("etc/hosts");
        let mut sb = String::new();
        sb.push_str("127.0.0.1\tlocalhost\n");
        sb.push_str("::1\tlocalhost ip6-localhost ip6-loopback\n\n");
        sb.push_str("# Zeno Container Service Discovery\n");

        if let Some(my_ip) = running_ips.get(&c.id) {
            sb.push_str(&format!("{}\t{}\n", my_ip, c.id));
        }

        let my_net = c.network.as_ref().map(|s| s.as_str()).unwrap_or("");
        for (other_id, other_ip) in &running_ips {
            if other_id != &c.id {
                let other_net = running_nets.get(other_id).map(|s| s.as_str()).unwrap_or("");
                if other_net == my_net {
                    sb.push_str(&format!("{}\t{}\n", other_ip, other_id));
                }
            }
        }

        let _ = fs::write(hosts_path, sb);
    }

    Ok(())
}

pub(crate) fn container_list_internal(data_dir: &str, auto_restart: bool) -> Result<Vec<ContainerState>, String> {
    let containers_dir = Path::new(data_dir).join("containers");
    if !containers_dir.exists() {
        return Ok(Vec::new());
    }

    let mut list = Vec::new();
    let entries = fs::read_dir(containers_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        if let Ok(entry) = entry {
            if entry.path().is_dir() {
                let id = entry.file_name().to_string_lossy().to_string();
                if let Ok(mut state) = load_container_state(&id) {
                    if state.status != "stopped" {
                        let output = runc_exec(&["state", &id]);
                        if let Ok(out) = output {
                            if out.status.success() {
                                let out_str = String::from_utf8_lossy(&out.stdout);
                                if let Ok(runc_st) = serde_json::from_str::<serde_json::Value>(&out_str) {
                                    let runc_status = runc_st.get("status").and_then(|s| s.as_str()).unwrap_or("stopped");
                                    let runc_pid = runc_st.get("pid").and_then(|p| p.as_i64()).unwrap_or(0) as i32;

                                    if state.status != runc_status || state.pid != runc_pid {
                                        state.status = runc_status.to_string();
                                        state.pid = runc_pid;
                                        if let Err(e) = save_container_state(&state) {
                                            eprintln!("  ⚠ Failed to save container state: {}", e);
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                if state.status == "running" || state.status == "created" {
                                    state.status = "stopped".to_string();
                                    state.pid = 0;
                                    if let Err(e) = save_container_state(&state) {
                                        eprintln!("  ⚠ Failed to save container state: {}", e);
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    // Enforce container restart policy (auto-restart on crash)
                    if auto_restart
                        && (state.status == "stopped" || state.status == "failed")
                        && state.desired_status.as_deref() == Some("running")
                    {
                        if let Some(ref policy) = state.restart_policy {
                            if policy == "always" || policy == "unless-stopped" {
                                eprintln!("🔄 [BoxSlot] Container {} is stopped but desired_status is 'running'. Policy: '{}'. Auto-restarting...", id, policy);
                                if let Err(e) = container_start(&id) {
                                    eprintln!("  ⚠ Auto-restart failed for container {}: {}", id, e);
                                } else if let Ok(new_state) = load_container_state(&id) {
                                    state = new_state;
                                }
                            }
                        }
                    }

                    list.push(state);
                }
            }
        }
    }

    Ok(list)
}

fn container_start(id: &str) -> Result<(), String> {
    let data_dir = get_data_dir();
    let mut state = load_container_state(id)?;
    if state.status == "running" {
        return Err(format!("Container {} is already running", id));
    }

    // Ensure overlayfs is mounted (mount can be lost after a system restart/reboot)
    mount_overlayfs(&state.image, &data_dir, id)?;

    // Clean up any residual network interfaces and rules from a previous crashed run
    let old_ip = state.env.as_ref().and_then(|e| e.get("ZENO_IP").cloned()).unwrap_or_default();
    let old_ports = state.ports.clone().unwrap_or_default();
    clean_container_network(id, &old_ip, &old_ports);

    let bundle_p = bundle_dir(&data_dir, id);

    let _ = runc_exec(&["delete", "--force", id]);

    let log_p = log_path(&data_dir, id);
    let log_file = File::create(&log_p).map_err(|e| format!("Failed to create log file: {}", e))?;

    let runc_bin = get_runc_bin();
    let root = format!("{}/runc", get_data_dir());
    let run_create_status = Command::new(&runc_bin)
        .args(&["--root", &root, "create", "-b", &bundle_p.to_string_lossy(), id])
        .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
        .stderr(log_file)
        .status()
        .map_err(|e| format!("runc create process failed: {}", e))?;

    if !run_create_status.success() {
        state.status = "failed".to_string();
        let _ = save_container_state(&state);
        let err_msg = fs::read_to_string(&log_p).unwrap_or_default();
        return Err(format!("runc create failed: {}", err_msg));
    }

    let mut runc_pid = 0;
    if let Ok(out) = runc_exec(&["state", id]) {
        if out.status.success() {
            let out_str = String::from_utf8_lossy(&out.stdout);
            if let Ok(runc_st) = serde_json::from_str::<serde_json::Value>(&out_str) {
                runc_pid = runc_st.get("pid").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
            }
        }
    }

    if runc_pid > 0 {
        state.pid = runc_pid;
        let is_host_net = state.host_network.unwrap_or(false);
        if !is_host_net {
            let ports = state.ports.clone().unwrap_or_default();
            let net_name = state.network.clone().unwrap_or_default();
            match configure_container_network(&data_dir, id, runc_pid, ports, &net_name) {
                Ok(ip) => {
                    let mut env = state.env.clone().unwrap_or_default();
                    env.insert("ZENO_IP".to_string(), ip);
                    state.env = Some(env);
                }
                Err(e) => {
                    eprintln!("  ⚠ Network configuration failed: {}", e);
                }
            }
        }
    }

    let run_start = runc_exec(&["start", id])
        .map_err(|e| format!("runc start process failed: {}", e))?;
    if !run_start.status.success() {
        // Clean up network interfaces and rules on startup failure
        let ip = state.env.as_ref().and_then(|e| e.get("ZENO_IP").cloned()).unwrap_or_default();
        let ports = state.ports.clone().unwrap_or_default();
        clean_container_network(id, &ip, &ports);

        state.status = "failed".to_string();
        let _ = save_container_state(&state);
        let err_msg = String::from_utf8_lossy(&run_start.stderr);
        return Err(format!("runc start failed: {}", err_msg));
    }

    state.status = "running".to_string();
    state.desired_status = Some("running".to_string());
    state.exit_code = Some(0);
    save_container_state(&state)?;

    let _ = sync_hosts_entries(&data_dir);
    Ok(())
}

fn container_stop(id: &str) -> Result<(), String> {
    let data_dir = get_data_dir();
    let mut state = load_container_state(id)?;
    if state.status != "running" {
        return Ok(());
    }

    let kill_term = runc_exec(&["kill", id, "SIGTERM"]);
    if kill_term.is_err() || !kill_term.unwrap().status.success() {
        let _ = runc_exec(&["kill", id, "SIGKILL"]);
    }

    let ip = state.env.as_ref().and_then(|e| e.get("ZENO_IP").cloned()).unwrap_or_default();
    let ports = state.ports.clone().unwrap_or_default();
    clean_container_network(id, &ip, &ports);

    state.status = "stopped".to_string();
    state.desired_status = Some("stopped".to_string());
    state.pid = 0;
    save_container_state(&state)?;

    let _ = sync_hosts_entries(&data_dir);
    Ok(())
}

fn container_delete(id: &str) -> Result<(), String> {
    let data_dir = get_data_dir();
    let state = load_container_state(id);
    if let Ok(state) = state {
        let ip = state.env.as_ref().and_then(|e| e.get("ZENO_IP").cloned()).unwrap_or_default();
        let ports = state.ports.clone().unwrap_or_default();
        clean_container_network(id, &ip, &ports);
    }

    let _ = runc_exec(&["kill", id, "SIGKILL"]);
    let _ = runc_exec(&["delete", "--force", id]);

    let dst_rootfs = rootfs_dir(&data_dir, id);
    if dst_rootfs.exists() {
        let _ = run_privileged_status("umount", &["-l", &dst_rootfs.to_string_lossy().to_string()]);
        // Give the kernel time to lazy-unmount the mount point
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let cont_p = container_dir(&data_dir, id);
    
    // Attempt deletion with retries to resolve transient locks/busy states
    let mut delete_err = None;
    for attempt in 1..=5 {
        match fs::remove_dir_all(&cont_p) {
            Ok(_) => {
                delete_err = None;
                break;
            }
            Err(e) => {
                delete_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(100 * attempt));
            }
        }
    }

    if let Some(err) = delete_err {
        return Err(format!("Failed to delete container directory '{}': {}", cont_p.display(), err));
    }

    let _ = sync_hosts_entries(&data_dir);
    Ok(())
}

fn container_update(id: &str, memory_limit: i64, cpu_limit: f64) -> Result<(), String> {
    let data_dir = get_data_dir();
    let mut state = load_container_state(id)?;

    let mut runc_args = vec!["update"];
    let mem_str = memory_limit.to_string();
    if memory_limit > 0 {
        runc_args.push("--memory");
        runc_args.push(&mem_str);
    }
    let period_str = "100000".to_string();
    let quota = (cpu_limit * 100000.0) as i64;
    let quota_str = quota.to_string();
    if cpu_limit > 0.0 {
        runc_args.push("--cpu-period");
        runc_args.push(&period_str);
        runc_args.push("--cpu-quota");
        runc_args.push(&quota_str);
    }
    runc_args.push(id);

    if state.status == "running" {
        let run_upd = runc_exec(&runc_args)
            .map_err(|e| format!("runc update failed: {}", e))?;
        if !run_upd.status.success() {
            let err_msg = String::from_utf8_lossy(&run_upd.stderr);
            return Err(format!("runc update failed: {}", err_msg));
        }
    }

    let config_path = bundle_dir(&data_dir, id).join("config.json");
    if config_path.exists() {
        if let Ok(data) = fs::read_to_string(&config_path) {
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&data) {
                if memory_limit > 0 {
                    val["linux"]["resources"]["memory"] = json!({ "limit": memory_limit });
                }
                if cpu_limit > 0.0 {
                    val["linux"]["resources"]["cpu"] = json!({
                        "period": 100000u64,
                        "quota": quota
                    });
                }
                if let Ok(new_data) = serde_json::to_string_pretty(&val) {
                    let _ = fs::write(&config_path, new_data);
                }
            }
        }
    }

    if memory_limit > 0 { state.memory_limit = Some(memory_limit); }
    if cpu_limit > 0.0 { state.cpu_limit = Some(cpu_limit); }
    save_container_state(&state)?;

    Ok(())
}

fn create_bridge_network(data_dir: &str, name: &str) -> Result<String, String> {
    let mut networks = get_networks(data_dir);
    for n in &networks {
        if n.name == name {
            return Err(format!("Network {} already exists", name));
        }
    }

    if name == "bridge" || name == "default" {
        return Err(format!("Network name {} is reserved", name));
    }

    let mut used_subnets = std::collections::HashSet::new();
    for n in &networks {
        let parts: Vec<&str> = n.subnet.split('.').collect();
        if parts.len() > 1 {
            if let Ok(x) = parts[1].parse::<i32>() {
                used_subnets.insert(x);
            }
        }
    }

    let mut selected_x = -1;
    for x in 21..=31 {
        if !used_subnets.contains(&x) {
            selected_x = x;
            break;
        }
    }

    if selected_x == -1 {
        return Err("No subnets available in 172.21.0.0/16 - 172.31.0.0/16".to_string());
    }

    let bridge_id = format!("zenobr{}", selected_x);
    let subnet = format!("172.{}.0.0/16", selected_x);
    let gateway = format!("172.{}.0.1", selected_x);

    let _ = run_cmd_status_silent("ip", &["link", "add", "name", &bridge_id, "type", "bridge"]);
    let _ = run_cmd_status_silent("ip", &["addr", "add", &format!("{}/16", gateway), "dev", &bridge_id]);
    let _ = run_cmd_status_silent("ip", &["link", "set", &bridge_id, "up"]);

    // Enable route_localnet on the custom bridge to allow loopback NAT MASQUERADE.
    let route_localnet_path = format!("/proc/sys/net/ipv4/conf/{}/route_localnet", bridge_id);
    let _ = std::fs::write(&route_localnet_path, "1");

    // Check if the NAT rule already exists before appending to prevent duplicates
    let rule_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-s", &subnet, "!", "-o", &bridge_id, "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);

    if !rule_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-s", &subnet, "!", "-o", &bridge_id, "-j", "MASQUERADE"]);
    }

    // Ensure FORWARD rules exist for this custom bridge so DNAT port-mapped traffic
    // can reach containers.
    let fwd_in_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-i", &bridge_id, "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_in_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-i", &bridge_id, "-j", "ACCEPT"]);
    }

    let fwd_out_exists = run_cmd_status_silent("iptables", &["-C", "FORWARD", "-o", &bridge_id, "-j", "ACCEPT"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !fwd_out_exists {
        let _ = run_cmd_status_silent("iptables", &["-A", "FORWARD", "-o", &bridge_id, "-j", "ACCEPT"]);
    }

    // Ensure loopback requests to port-forwarded ports on this custom bridge are masqueraded.
    let local_masq_exists = run_cmd_status_silent("iptables", &["-t", "nat", "-C", "POSTROUTING", "-o", &bridge_id, "-s", "127.0.0.1", "-j", "MASQUERADE"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !local_masq_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-A", "POSTROUTING", "-o", &bridge_id, "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    }

    // Always ensure CHECKSUM fill rule exists for this custom bridge.
    let chk_exists = run_cmd_status_silent("iptables", &["-t", "mangle", "-C", "POSTROUTING", "-o", &bridge_id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"])
        .map(|s| s.success())
        .unwrap_or(false);
    if !chk_exists {
        let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-A", "POSTROUTING", "-o", &bridge_id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    }

    let new_net = NetworkConfig {
        id: bridge_id.clone(),
        name: name.to_string(),
        driver: "bridge".to_string(),
        subnet,
        gateway,
    };
    networks.push(new_net);
    save_networks(data_dir, &networks)?;

    Ok(bridge_id)
}

fn delete_bridge_network(data_dir: &str, name: &str) -> Result<(), String> {
    let mut networks = get_networks(data_dir);
    let mut found_idx = None;
    for (i, n) in networks.iter().enumerate() {
        if n.name == name || n.id == name {
            found_idx = Some(i);
            break;
        }
    }

    let idx = found_idx.ok_or_else(|| format!("Network {} not found", name))?;
    let net = &networks[idx];

    let containers = container_list_internal(data_dir, false)?;
    for c in containers {
        if c.network.as_ref().map(|s| s == name || s == &net.id).unwrap_or(false) && c.status == "running" {
            return Err(format!("Network is in use by running container {}", c.id));
        }
    }

    let _ = run_cmd_status_silent("ip", &["link", "set", &net.id, "down"]);
    let _ = run_cmd_status_silent("ip", &["link", "delete", &net.id]);
    let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-D", "POSTROUTING", "-s", &net.subnet, "!", "-o", &net.id, "-j", "MASQUERADE"]);
    let _ = run_cmd_status_silent("iptables", &["-t", "nat", "-D", "POSTROUTING", "-o", &net.id, "-s", "127.0.0.1", "-j", "MASQUERADE"]);
    let _ = run_cmd_status_silent("iptables", &["-t", "mangle", "-D", "POSTROUTING", "-o", &net.id, "-p", "tcp", "-j", "CHECKSUM", "--fill-checksum"]);
    let _ = run_cmd_status_silent("iptables", &["-D", "FORWARD", "-i", &net.id, "-j", "ACCEPT"]);
    let _ = run_cmd_status_silent("iptables", &["-D", "FORWARD", "-o", &net.id, "-j", "ACCEPT"]);

    networks.remove(idx);
    save_networks(data_dir, &networks)?;

    Ok(())
}

fn register_box_pull(engine: &mut Engine) {
    engine.register(
        "box.pull",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "pull_result".to_string();

            if node.value.is_some() {
                let resolved = resolve_node_value(_engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                if child.name == "image" {
                    image = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let rt = tokio::runtime::Handle::current();
            let res = tokio::task::block_in_place(|| {
                rt.block_on(async { pull_image_rust(&image).await })
            });

            let mut result = HashMap::new();
            match res {
                Ok(_cmd) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String("Image pulled successfully".to_string()));
                    result.insert("stderr".to_string(), Value::String(String::new()));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stdout".to_string(), Value::String(String::new()));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Pull container image natively in Rust".to_string(),
            example: "box.pull: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_images(engine: &mut Engine) {
    engine.register(
        "box.images",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "images".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let images_dir = Path::new(&data_dir).join("images");
            let mut images = Vec::new();
            if let Ok(entries) = fs::read_dir(images_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if entry.path().is_dir() && entry.file_name() != "layers" {
                            let name = entry.file_name().to_string_lossy().to_string()
                                .replace('_', "/");
                            if let Some(idx) = name.rfind('/') {
                                let (repo, tag) = name.split_at(idx);
                                let tag_clean = tag.trim_start_matches('/');
                                images.push(Value::String(format!("{}:{}", repo, tag_clean)));
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(images));
            Ok(())
        }),
        SlotMeta {
            description: "List cached container images".to_string(),
            example: "box.images { as: $images }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn prune_unused_layers() -> io::Result<()> {
    let data_dir = get_data_dir();
    let images_dir = Path::new(&data_dir).join("images");
    if !images_dir.exists() {
        return Ok(());
    }

    let mut used_layers = std::collections::HashSet::new();

    // 1. Scan image metadata directories to find used layers
    if let Ok(entries) = fs::read_dir(&images_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() && entry.file_name() != "layers" {
                    let layers_json_path = path.join("layers.json");
                    if layers_json_path.exists() {
                        if let Ok(file) = File::open(&layers_json_path) {
                            if let Ok(layers) = serde_json::from_reader::<_, Vec<String>>(file) {
                                for layer in layers {
                                    used_layers.insert(layer);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Scan layers directory and delete unused ones
    let layers_dir = images_dir.join("layers");
    if layers_dir.exists() {
        if let Ok(entries) = fs::read_dir(&layers_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let layer_name = entry.file_name().to_string_lossy().to_string();
                        if !used_layers.contains(&layer_name) {
                            let _ = fs::remove_dir_all(&path);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn register_box_prune(engine: &mut Engine) {
    engine.register(
        "box.prune",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "prune_result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let mut result = HashMap::new();
            match prune_unused_layers() {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e.to_string()));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Prune unused OCI image layers".to_string(),
            example: "box.prune: { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_rmi(engine: &mut Engine) {
    engine.register(
        "box.rmi",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "rmi_result".to_string();

            if node.value.is_some() {
                let resolved = resolve_node_value(_engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                if child.name == "image" {
                    image = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let img_ref = parse_image_ref(&image);
            let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
                .replace('/', "_")
                .replace(':', "_");
            let data_dir = get_data_dir();
            let cache_dir = Path::new(&data_dir).join("images").join(cache_dir_name);

            let mut result = HashMap::new();
            if cache_dir.exists() {
                if let Err(e) = fs::remove_dir_all(cache_dir) {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e.to_string()));
                } else {
                    let _ = prune_unused_layers();
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String("Image removed".to_string()));
                }
            } else {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String("Image not found".to_string()));
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Remove a cached image".to_string(),
            example: "box.rmi: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_create(engine: &mut Engine) {
    engine.register(
        "box.create",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut image = String::new();
            let mut cmd_vec = Vec::new();
            let mut cmd_specified = false;
            let mut ports = Vec::new();
            let mut volumes = Vec::new();
            let mut env_map = HashMap::new();
            let mut host_net = false;
            let mut memory = String::new();
            let mut cpus = String::new();
            let mut oom_score_adj_str = String::new();
            let mut read_only = false;
            let mut target = "create_result".to_string();

            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                } else {
                    let resolved = resolve_node_value(_engine, child, scope);
                    match child.name.as_str() {
                        "name" => name = resolved.to_string_coerce(),
                        "image" => image = resolved.to_string_coerce(),
                        "cmd" => {
                            cmd_specified = true;
                            if let Value::List(ref list) = resolved {
                                cmd_vec = list.iter().map(|v| v.to_string_coerce()).collect();
                            } else {
                                let val_str = resolved.to_string_coerce();
                                if !val_str.is_empty() {
                                    cmd_vec = val_str.split_whitespace().map(|s| s.to_string()).collect();
                                }
                            }
                        }
                        "host_net" => host_net = resolved.to_bool(),
                        "memory" => memory = resolved.to_string_coerce(),
                        "cpus" => cpus = resolved.to_string_coerce(),
                        "oom_score_adj" => oom_score_adj_str = resolved.to_string_coerce(),
                        "read_only" => read_only = resolved.to_bool(),
                        "ports" => {
                            if let Value::List(ref list) = resolved {
                                ports = list.iter().map(|v| v.to_string_coerce()).collect();
                            } else {
                                let val_str = resolved.to_string_coerce();
                                if !val_str.is_empty() {
                                    ports.push(val_str);
                                }
                            }
                        }
                        "volumes" => {
                            if let Value::List(ref list) = resolved {
                                volumes = list.iter().map(|v| v.to_string_coerce()).collect();
                            } else {
                                let val_str = resolved.to_string_coerce();
                                if !val_str.is_empty() {
                                    volumes.push(val_str);
                                }
                            }
                        }
                        "env" => {
                            if let Value::Map(ref map) = resolved {
                                for (k, v) in map {
                                    env_map.insert(k.clone(), v.to_string_coerce());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            let mem_bytes = if memory.is_empty() { 0 } else {
                if memory.ends_with('m') || memory.ends_with('M') {
                    memory[..memory.len()-1].parse::<i64>().unwrap_or(0) * 1024 * 1024
                } else if memory.ends_with('g') || memory.ends_with('G') {
                    memory[..memory.len()-1].parse::<i64>().unwrap_or(0) * 1024 * 1024 * 1024
                } else {
                    memory.parse::<i64>().unwrap_or(0)
                }
            };

            let cpu_limit = cpus.parse::<f64>().unwrap_or(0.0);
            let oom_score_adj = oom_score_adj_str.parse::<i32>().ok();

            let cmd_vec = if !cmd_specified || cmd_vec.is_empty() {
                get_image_default_cmd(&image)
            } else {
                cmd_vec
            };

            let res = container_create(
                &name,
                &image,
                cmd_vec,
                env_map,
                "",
                volumes,
                ports,
                host_net,
                "always",
                mem_bytes,
                cpu_limit,
                oom_score_adj,
                read_only,
                "bridge",
            );

            let mut result = HashMap::new();
            match res {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Create a box container".to_string(),
            example: "box.create { name: 'web', image: 'nginx:alpine' }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn resolve_id_param(engine: &Engine, node: &zenocore::Node, scope: &Arc<zenocore::Scope>) -> (String, String) {
    let mut id = String::new();
    let mut target = String::new();
    if node.value.is_some() {
        id = resolve_node_value(engine, node, scope).to_string_coerce();
    }
    for child in &node.children {
        if child.name == "id" {
            id = resolve_node_value(engine, child, scope).to_string_coerce();
        } else if child.name == "as" {
            if let Some(ref val) = child.value {
                target = val.trim_start_matches('$').to_string();
            }
        }
    }
    (id, target)
}

fn register_box_start(engine: &mut Engine) {
    engine.register(
        "box.start",
        Arc::new(|_engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(_engine, node, scope);
            if target.is_empty() {
                target = "start_result".to_string();
            }

            let mut result = HashMap::new();
            match container_start(&id) {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Start a box container".to_string(),
            example: "box.start: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_stop(engine: &mut Engine) {
    engine.register(
        "box.stop",
        Arc::new(|_engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(_engine, node, scope);
            if target.is_empty() {
                target = "stop_result".to_string();
            }

            let mut result = HashMap::new();
            match container_stop(&id) {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Stop a running box container".to_string(),
            example: "box.stop: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_delete(engine: &mut Engine) {
    engine.register(
        "box.delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(_engine, node, scope);
            if target.is_empty() {
                target = "delete_result".to_string();
            }

            let mut result = HashMap::new();
            match container_delete(&id) {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Delete a box container".to_string(),
            example: "box.delete: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_list(engine: &mut Engine) {
    engine.register(
        "box.list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "containers".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let mut list_value = Vec::new();
            if let Ok(containers) = container_list_internal(&data_dir, false) {
                for c in containers {
                    let mut m = HashMap::new();
                    m.insert("id".to_string(), Value::String(c.id.clone()));
                    m.insert("image".to_string(), Value::String(c.image.clone()));
                    m.insert("status".to_string(), Value::String(c.status.clone()));
                    m.insert("pid".to_string(), Value::Int(c.pid as i64));
                    m.insert("created_at".to_string(), Value::String(c.created_at.clone()));
                    m.insert("cmd".to_string(), Value::String(c.cmd.join(" ")));
                    
                    let state_str = if c.status == "running" { "running" } else { "exited" };
                    m.insert("state".to_string(), Value::String(state_str.to_string()));
                    
                    let mut port_vals = Vec::new();
                    if let Some(ports) = c.ports {
                        for p in ports {
                            port_vals.push(Value::String(p));
                        }
                    }
                    m.insert("ports".to_string(), Value::List(port_vals));

                    list_value.push(Value::Map(m));
                }
            }

            scope.set(&target, Value::List(list_value));
            Ok(())
        }),
        SlotMeta {
            description: "List all box containers".to_string(),
            example: "box.list { as: $containers }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_inspect(engine: &mut Engine) {
    engine.register(
        "box.inspect",
        Arc::new(|_engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(_engine, node, scope);
            if target.is_empty() {
                target = "inspect_result".to_string();
            }

            let mut result = HashMap::new();
            match load_container_state(&id) {
                Ok(state) => {
                    let state_str = serde_json::to_string_pretty(&state).unwrap_or_default();
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String(state_str.clone()));
                    result.insert("exit_code".to_string(), Value::Int(0));
                    
                    let value = serde_json_to_zeno(&serde_json::to_value(&state).unwrap_or(serde_json::Value::Null));
                    result.insert("data".to_string(), value);
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                    result.insert("data".to_string(), Value::Nil);
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Inspect container details".to_string(),
            example: "box.inspect: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_logs(engine: &mut Engine) {
    engine.register(
        "box.logs",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut id = String::new();
            let mut tail = 0;
            let mut target = "logs_result".to_string();

            if node.value.is_some() {
                id = resolve_node_value(_engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "id" => id = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "tail" => tail = resolve_node_value(_engine, child, scope).to_int(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let log_p = log_path(&data_dir, &id);

            let mut result = HashMap::new();
            if log_p.exists() {
                if let Ok(file) = File::open(log_p) {
                    let reader = BufReader::new(file);
                    let mut lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
                    if tail > 0 && lines.len() > tail as usize {
                        lines = lines[lines.len() - tail as usize..].to_vec();
                    }
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String(lines.join("\n")));
                    result.insert("exit_code".to_string(), Value::Int(0));
                } else {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String("Failed to read log file".to_string()));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                }
            } else {
                result.insert("success".to_string(), Value::Bool(true));
                result.insert("stdout".to_string(), Value::String("No logs available".to_string()));
                result.insert("exit_code".to_string(), Value::Int(0));
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Get container console logs".to_string(),
            example: "box.logs: 'my-web' { tail: 50, as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_rootfs_path(engine: &mut Engine) {
    engine.register(
        "box.rootfs_path",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut container_id = String::new();
            let mut target = "rootfs_path".to_string();

            if node.value.is_some() {
                let resolved = resolve_node_value(_engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    container_id = val_str;
                }
            }

            for child in &node.children {
                if child.name == "id" {
                    container_id = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let path = rootfs_dir(&get_data_dir(), &container_id).to_string_lossy().to_string();
            scope.set(&target, Value::String(path));
            Ok(())
        }),
        SlotMeta {
            description: "Get container rootfs path".to_string(),
            example: "box.rootfs_path: 'my-container' { as: $path }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_update(engine: &mut Engine) {
    engine.register(
        "box.update",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut id = String::new();
            let mut memory = String::new();
            let mut cpus = String::new();
            let mut target = "update_result".to_string();

            if node.value.is_some() {
                id = resolve_node_value(_engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "id" => id = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "memory" => memory = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "cpus" => cpus = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let mem_bytes = if memory.is_empty() { 0 } else {
                if memory.ends_with('m') || memory.ends_with('M') {
                    memory[..memory.len()-1].parse::<i64>().unwrap_or(0) * 1024 * 1024
                } else if memory.ends_with('g') || memory.ends_with('G') {
                    memory[..memory.len()-1].parse::<i64>().unwrap_or(0) * 1024 * 1024 * 1024
                } else {
                    memory.parse::<i64>().unwrap_or(0)
                }
            };
            let cpu_limit = cpus.parse::<f64>().unwrap_or(0.0);

            let mut result = HashMap::new();
            match container_update(&id, mem_bytes, cpu_limit) {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Update container resource limits".to_string(),
            example: "box.update: 'my-web' { memory: '1g', cpus: '2', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_volume_list(engine: &mut Engine) {
    engine.register(
        "box.volume_list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "volumes".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let volumes_dir = Path::new(&data_dir).join("volumes");
            let _ = fs::create_dir_all(&volumes_dir);

            let mut list = Vec::new();
            if let Ok(entries) = fs::read_dir(volumes_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if entry.path().is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let mut m = HashMap::new();
                            m.insert("Name".to_string(), Value::String(name.clone()));
                            m.insert("Driver".to_string(), Value::String("local".to_string()));
                            m.insert("Mountpoint".to_string(), Value::String(entry.path().to_string_lossy().to_string()));
                            list.push(Value::Map(m));
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List storage volumes".to_string(),
            example: "box.volume_list { as: $volumes }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_volume_create(engine: &mut Engine) {
    engine.register(
        "box.volume_create",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "volume_create_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let vol_p = Path::new(&data_dir).join("volumes").join(&name);
            let res = fs::create_dir_all(&vol_p);

            let mut result = HashMap::new();
            match res {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e.to_string()));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Create storage volume".to_string(),
            example: "box.volume_create { name: 'my-vol', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_volume_delete(engine: &mut Engine) {
    engine.register(
        "box.volume_delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "volume_delete_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let vol_p = Path::new(&data_dir).join("volumes").join(&name);
            let res = fs::remove_dir_all(&vol_p);

            let mut result = HashMap::new();
            match res {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e.to_string()));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Delete storage volume".to_string(),
            example: "box.volume_delete { name: 'my-vol', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_network_list(engine: &mut Engine) {
    engine.register(
        "box.network_list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "networks".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let mut list = Vec::new();

            let mut default_net = HashMap::new();
            default_net.insert("Name".to_string(), Value::String("bridge".to_string()));
            default_net.insert("Id".to_string(), Value::String("zenobr0".to_string()));
            default_net.insert("Driver".to_string(), Value::String("bridge".to_string()));
            default_net.insert("Subnet".to_string(), Value::String("172.20.0.0/16".to_string()));
            default_net.insert("Gateway".to_string(), Value::String("172.20.0.1".to_string()));
            list.push(Value::Map(default_net));

            let custom_nets = get_networks(&data_dir);
            for n in custom_nets {
                let mut m = HashMap::new();
                m.insert("Name".to_string(), Value::String(n.name));
                m.insert("Id".to_string(), Value::String(n.id));
                m.insert("Driver".to_string(), Value::String(n.driver));
                m.insert("Subnet".to_string(), Value::String(n.subnet));
                m.insert("Gateway".to_string(), Value::String(n.gateway));
                list.push(Value::Map(m));
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List bridge networks".to_string(),
            example: "box.network_list { as: $networks }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_network_create(engine: &mut Engine) {
    engine.register(
        "box.network_create",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "network_create_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let res = create_bridge_network(&data_dir, &name);

            let mut result = HashMap::new();
            match res {
                Ok(id) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("id".to_string(), Value::String(id));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Create bridge network".to_string(),
            example: "box.network_create { name: 'my-net', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_network_delete(engine: &mut Engine) {
    engine.register(
        "box.network_delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut target = "network_delete_result".to_string();

            for child in &node.children {
                if child.name == "name" {
                    name = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let res = delete_bridge_network(&data_dir, &name);

            let mut result = HashMap::new();
            match res {
                Ok(_) => {
                    result.insert("success".to_string(), Value::Bool(true));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Delete bridge network".to_string(),
            example: "box.network_delete { name: 'my-net', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposeExtraHosts(pub Vec<String>);

impl<'de> serde::Deserialize<'de> for ComposeExtraHosts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ExtraHostsVisitor;
        impl<'de> serde::de::Visitor<'de> for ExtraHostsVisitor {
            type Value = ComposeExtraHosts;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of strings or a map of hostnames to IPs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut hosts = Vec::new();
                while let Some(elem) = seq.next_element::<String>()? {
                    hosts.push(elem);
                }
                Ok(ComposeExtraHosts(hosts))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut hosts = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    hosts.push(format!("{}:{}", k, v));
                }
                Ok(ComposeExtraHosts(hosts))
            }
        }
        deserializer.deserialize_any(ExtraHostsVisitor)
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposeCommand(pub Vec<String>);

impl<'de> serde::Deserialize<'de> for ComposeCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CmdVisitor;
        impl<'de> serde::de::Visitor<'de> for CmdVisitor {
            type Value = ComposeCommand;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a sequence of strings")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ComposeCommand(v.split_whitespace().map(|s| s.to_string()).collect()))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut cmd = Vec::new();
                while let Some(elem) = seq.next_element::<String>()? {
                    cmd.push(elem);
                }
                Ok(ComposeCommand(cmd))
            }
        }
        deserializer.deserialize_any(CmdVisitor)
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposeEnvironment(pub HashMap<String, String>);

impl<'de> serde::Deserialize<'de> for ComposeEnvironment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EnvVisitor;
        impl<'de> serde::de::Visitor<'de> for EnvVisitor {
            type Value = ComposeEnvironment;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map or a sequence of strings")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut env = HashMap::new();
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    let v_str = match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => String::new(),
                        _ => v.to_string(),
                    };
                    env.insert(k, v_str);
                }
                Ok(ComposeEnvironment(env))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut env = HashMap::new();
                while let Some(item) = seq.next_element::<serde_json::Value>()? {
                    let item_str = match item {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    let parts: Vec<&str> = item_str.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        env.insert(parts[0].to_string(), parts[1].to_string());
                    } else if parts.len() == 1 {
                        env.insert(parts[0].to_string(), String::new());
                    }
                }
                Ok(ComposeEnvironment(env))
            }
        }
        deserializer.deserialize_any(EnvVisitor)
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposePorts(pub Vec<String>);

impl<'de> serde::Deserialize<'de> for ComposePorts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PortsVisitor;
        impl<'de> serde::de::Visitor<'de> for PortsVisitor {
            type Value = ComposePorts;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of strings, integers, or objects mapping ports")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut ports = Vec::new();
                while let Some(elem) = seq.next_element::<serde_json::Value>()? {
                    match elem {
                        serde_json::Value::String(s) => ports.push(s),
                        serde_json::Value::Number(n) => ports.push(n.to_string()),
                        serde_json::Value::Object(obj) => {
                            let target = obj.get("target")
                                .map(|v| match v {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => String::new(),
                                })
                                .unwrap_or_default();
                            
                            let published = obj.get("published")
                                .map(|v| match v {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => String::new(),
                                });

                            let host_ip = obj.get("host_ip")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            let protocol = obj.get("protocol")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            if !target.is_empty() {
                                let mut port_str = String::new();
                                if let Some(ip) = host_ip {
                                    port_str.push_str(&format!("{}:", ip));
                                }
                                if let Some(pub_port) = published {
                                    port_str.push_str(&format!("{}:", pub_port));
                                }
                                port_str.push_str(&target);
                                if let Some(proto) = protocol {
                                    port_str.push_str(&format!("/{}", proto));
                                }
                                ports.push(port_str);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(ComposePorts(ports))
            }
        }
        deserializer.deserialize_seq(PortsVisitor)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComposeHealthCheck {
    pub test: serde_yaml::Value,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub retries: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComposeService {
    pub image: Option<String>,
    pub container_name: Option<String>,
    pub ports: Option<ComposePorts>,
    pub environment: Option<ComposeEnvironment>,
    pub env_file: Option<serde_yaml::Value>,
    pub volumes: Option<Vec<String>>,
    pub entrypoint: Option<ComposeCommand>,
    pub command: Option<ComposeCommand>,
    pub depends_on: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthCheck>,
    pub mem_limit: Option<String>,
    pub cpus: Option<f64>,
    pub oom_score_adj: Option<i32>,
    pub read_only: Option<bool>,
    pub extra_hosts: Option<ComposeExtraHosts>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComposeNetwork {
    pub driver: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComposeFile {
    pub version: Option<String>,
    pub services: HashMap<String, ComposeService>,
    pub networks: Option<HashMap<String, ComposeNetwork>>,
    pub volumes: Option<HashMap<String, serde_yaml::Value>>,
}

fn order_services(services: &HashMap<String, ComposeService>) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut visited = std::collections::HashSet::new();

    fn visit(
        name: &str,
        services: &HashMap<String, ComposeService>,
        visited: &mut std::collections::HashSet<String>,
        ordered: &mut Vec<String>,
    ) {
        if visited.contains(name) {
            return;
        }
        visited.insert(name.to_string());
        if let Some(svc) = services.get(name) {
            if let Some(ref deps) = svc.depends_on {
                for dep in deps {
                    if services.contains_key(dep) {
                        visit(dep, services, visited, ordered);
                    }
                }
            }
        }
        ordered.push(name.to_string());
    }

    let mut keys: Vec<String> = services.keys().cloned().collect();
    keys.sort();

    for k in keys {
        visit(&k, services, &mut visited, &mut ordered);
    }

    ordered
}

fn parse_memory_bytes(m_str: &str) -> i64 {
    if m_str.is_empty() {
        return 0;
    }
    let clean = m_str.trim().to_lowercase();
    let mut unit: i64 = 1;
    let mut num_str = clean.as_str();
    if num_str.ends_with('b') {
        num_str = &num_str[..num_str.len() - 1];
    }
    if num_str.ends_with('k') {
        unit = 1024;
        num_str = &num_str[..num_str.len() - 1];
    } else if num_str.ends_with('m') {
        unit = 1024 * 1024;
        num_str = &num_str[..num_str.len() - 1];
    } else if num_str.ends_with('g') {
        unit = 1024 * 1024 * 1024;
        num_str = &num_str[..num_str.len() - 1];
    }

    num_str.parse::<i64>().unwrap_or(0) * unit
}

#[derive(Debug, Clone)]
pub struct PortRule {
    pub host_ip: Option<String>,
    pub host_port: String,
    pub container_port: String,
    pub protocol: String, // "tcp" or "udp"
}

pub fn parse_port_rule(p: &str) -> Option<PortRule> {
    let (clean_p, protocol) = if p.ends_with("/udp") {
        (&p[..p.len() - 4], "udp".to_string())
    } else if p.ends_with("/tcp") {
        (&p[..p.len() - 4], "tcp".to_string())
    } else {
        (p, "tcp".to_string())
    };

    let parts: Vec<&str> = clean_p.split(':').collect();
    match parts.len() {
        3 => {
            Some(PortRule {
                host_ip: Some(parts[0].to_string()),
                host_port: parts[1].to_string(),
                container_port: parts[2].to_string(),
                protocol,
            })
        }
        2 => {
            Some(PortRule {
                host_ip: None,
                host_port: parts[0].to_string(),
                container_port: parts[1].to_string(),
                protocol,
            })
        }
        1 => {
            if parts[0].is_empty() {
                None
            } else {
                Some(PortRule {
                    host_ip: None,
                    host_port: parts[0].to_string(),
                    container_port: parts[0].to_string(),
                    protocol,
                })
            }
        }
        _ => None,
    }
}

fn load_env_file(compose_path: &str, env_file_val: &serde_yaml::Value) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let compose_path_buf = Path::new(compose_path);
    let parent_dir = compose_path_buf.parent().unwrap_or_else(|| Path::new("."));

    let files = match env_file_val {
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Sequence(seq) => {
            let mut v = Vec::new();
            for item in seq {
                if let Some(s) = item.as_str() {
                    v.push(s.to_string());
                }
            }
            v
        }
        _ => Vec::new(),
    };

    for file_name in files {
        let f_path = parent_dir.join(&file_name);
        if let Ok(content) = fs::read_to_string(f_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let k = parts[0].trim().to_string();
                    let v = parts[1].trim().to_string();
                    let v_clean = if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
                        if v.len() >= 2 {
                            v[1..v.len()-1].to_string()
                        } else {
                            v
                        }
                    } else {
                        v
                    };
                    env.insert(k, v_clean);
                }
            }
        }
    }

    env
}

fn inject_hosts_entries(
    data_dir: &str,
    container_id: &str,
    services: &HashMap<String, ComposeService>,
    current_name: &str,
) -> Result<(), String> {
    let hosts_path = rootfs_dir(data_dir, container_id).join("etc/hosts");
    let mut data = fs::read_to_string(&hosts_path).unwrap_or_else(|_| "127.0.0.1 localhost\n".to_string());

    let mut entries = Vec::new();
    for (svc_name, svc) in services {
        if svc_name == current_name {
            continue;
        }
        let cn = svc.container_name.as_ref().unwrap_or(svc_name);
        entries.push(format!("127.0.0.1\t{}\t{}", cn, svc_name));
    }

    if let Some(svc) = services.get(current_name) {
        if let Some(ref eh) = svc.extra_hosts {
            for entry in &eh.0 {
                let parts: Vec<&str> = entry.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let hostname = parts[0].trim();
                    let ip = parts[1].trim();
                    entries.push(format!("{}\t{}", ip, hostname));
                }
            }
        }
    }

    if entries.is_empty() {
        return Ok(());
    }

    data.push_str("\n# ZenoPanel compose service discovery\n");
    for e in entries {
        data.push_str(&format!("{}\n", e));
    }

    fs::write(hosts_path, data).map_err(|e| e.to_string())?;
    Ok(())
}

fn compose_up(path: &str) -> Result<String, String> {
    let data_dir = get_data_dir();
    let f = File::open(path).map_err(|e| format!("Failed to read compose file: {}", e))?;
    let cf: ComposeFile = serde_yaml::from_reader(f).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    let compose_path_buf = Path::new(path);
    let project_name = compose_path_buf
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "default".to_string());

    let ordered = order_services(&cf.services);
    let mut output = String::new();

    for name in ordered {
        let svc = &cf.services[&name];
        output.push_str(&format!("▶ Service: {} (image: {:?})\n", name, svc.image));

        let image = svc.image.as_ref().ok_or_else(|| format!("Service {} has no image", name))?;
        let img_ref = parse_image_ref(image);
        let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
            .replace('/', "_")
            .replace(':', "_");
        let cache_dir = Path::new(&data_dir).join("images").join(&cache_dir_name);
        let default_cmd = if !cache_dir.exists() {
            output.push_str(&format!("  ▶ Image {} not found locally. Pulling...\n", image));
            let rt = tokio::runtime::Handle::current();
            let pull_res = tokio::task::block_in_place(|| {
                rt.block_on(async { pull_image_rust(image).await })
            });
            match pull_res {
                Ok(cmd) => cmd,
                Err(e) => {
                    return Err(format!("Failed to pull image {}: {}", image, e));
                }
            }
        } else {
            get_image_default_cmd(image)
        };

        let container_name = svc.container_name.as_ref().unwrap_or(&name);

        let cont_p = container_dir(&data_dir, container_name);
        if cont_p.exists() {
            output.push_str(&format!("  ▶ Container '{}' already exists. Stopping and removing first...\n", container_name));
            let _ = container_stop(container_name);
            let _ = container_delete(container_name);
        }

        let cmd_args = match (&svc.entrypoint, &svc.command) {
            (Some(entrypoint), Some(command)) => {
                let mut cmd = entrypoint.0.clone();
                cmd.extend(command.0.clone());
                cmd
            }
            (Some(entrypoint), None) => {
                entrypoint.0.clone()
            }
            (None, Some(command)) => {
                command.0.clone()
            }
            (None, None) => {
                default_cmd
            }
        };

        let mut env = if let Some(ref e) = svc.environment {
            e.0.clone()
        } else {
            HashMap::new()
        };

        if let Some(ref env_file_val) = svc.env_file {
            let loaded_env = load_env_file(path, env_file_val);
            for (k, v) in loaded_env {
                env.entry(k).or_insert(v);
            }
        }

        let mut volumes = Vec::new();
        if let Some(ref vols) = svc.volumes {
            for v in vols {
                let parts: Vec<&str> = v.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let host_path = parts[0];
                    let container_path = parts[1];
                    let is_named_volume = !host_path.starts_with('/') && !host_path.starts_with('.') && !host_path.starts_with('~');
                    if is_named_volume {
                        volumes.push(format!("{}_{}:{}", project_name, host_path, container_path));
                    } else {
                        // Resolve relative paths (starting with .) relative to the directory containing the docker-compose.yml file.
                        // This mirrors Docker Compose behavior where relative mounts are resolved relative to the compose file.
                        let resolved_path = if host_path.starts_with('.') {
                            if let Some(parent) = compose_path_buf.parent() {
                                let abs_path = parent.join(host_path);
                                if let Ok(canonical) = fs::canonicalize(&abs_path) {
                                    canonical.to_string_lossy().to_string()
                                } else {
                                    abs_path.to_string_lossy().to_string()
                                }
                            } else {
                                host_path.to_string()
                            }
                        } else {
                            host_path.to_string()
                        };
                        volumes.push(format!("{}:{}", resolved_path, container_path));
                    }
                } else {
                    volumes.push(v.clone());
                }
            }
        }

        let ports = if let Some(ref p) = svc.ports {
            p.0.clone()
        } else {
            Vec::new()
        };

        let restart_policy = svc.restart.as_deref().unwrap_or("no");
        let mem_limit = if let Some(ref limit) = svc.mem_limit {
            parse_memory_bytes(limit)
        } else {
            0
        };
        let cpu_limit = svc.cpus.unwrap_or(0.0);
        let read_only = svc.read_only.unwrap_or(false);
        let network_name = if let Some(ref nets) = svc.networks {
            if !nets.is_empty() {
                &nets[0]
            } else {
                "bridge"
            }
        } else {
            "bridge"
        };

        output.push_str(&format!("  ▶ Creating container '{}'...\n", container_name));
        container_create(
            container_name,
            image,
            cmd_args,
            env,
            "",
            volumes,
            ports,
            false,
            restart_policy,
            mem_limit,
            cpu_limit,
            svc.oom_score_adj,
            read_only,
            network_name,
        )?;

        if let Err(e) = inject_hosts_entries(&data_dir, container_name, &cf.services, &name) {
            output.push_str(&format!("  ⚠ Warning: could not inject hosts: {}\n", e));
        }

        output.push_str(&format!("  ▶ Starting container '{}'...\n", container_name));
        container_start(container_name)?;
        output.push_str(&format!("  ✓ Service '{}' is up.\n", name));
    }

    Ok(output)
}

fn compose_down(path: &str) -> Result<String, String> {
    let f = File::open(path).map_err(|e| format!("Failed to read compose file: {}", e))?;
    let cf: ComposeFile = serde_yaml::from_reader(f).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    let ordered = order_services(&cf.services);
    let mut output = String::new();

    for name in ordered.iter().rev() {
        let svc = &cf.services[name];
        let container_name = svc.container_name.as_ref().unwrap_or(name);

        output.push_str(&format!("  ▶ Stopping container '{}'...\n", container_name));
        if let Err(e) = container_stop(container_name) {
            output.push_str(&format!("  ⚠ Error stopping {}: {}\n", container_name, e));
        }

        output.push_str(&format!("  ▶ Removing container '{}'...\n", container_name));
        if let Err(e) = container_delete(container_name) {
            output.push_str(&format!("  ⚠ Error removing {}: {}\n", container_name, e));
        }
    }

    Ok(output)
}

fn compose_ps(path: &str) -> Result<String, String> {
    let data_dir = get_data_dir();
    let f = File::open(path).map_err(|e| format!("Failed to read compose file: {}", e))?;
    let cf: ComposeFile = serde_yaml::from_reader(f).map_err(|e| format!("Failed to parse YAML: {}", e))?;

    let containers = container_list_internal(&data_dir, false)?;
    
    let mut expected = HashMap::new();
    for (svc_name, svc) in &cf.services {
        let cn = svc.container_name.as_ref().unwrap_or(svc_name);
        expected.insert(cn.clone(), svc_name.clone());
    }

    let mut matched = Vec::new();
    for c in containers {
        if expected.contains_key(&c.id) {
            matched.push(c);
        }
    }

    if matched.is_empty() {
        return Ok("No containers found for this compose file.".to_string());
    }

    let mut out = format!("{:<8} {:<24} {:<24} {:<10} {:<8} {}\n", "SERVICE", "CONTAINER", "IMAGE", "STATUS", "PID", "PORTS");
    out.push_str(&"-".repeat(110));
    out.push('\n');

    for c in matched {
        let svc_name = &expected[&c.id];
        let ports = c.ports.map(|p| p.join(",")).unwrap_or_default();
        let ports_str = if ports.is_empty() { "-" } else { &ports };
        let pid_str = if c.pid > 0 { c.pid.to_string() } else { "-".to_string() };
        
        out.push_str(&format!("{:<8} {:<24} {:<24} {:<10} {:<8} {}\n", svc_name, c.id, c.image, c.status, pid_str, ports_str));
    }

    Ok(out)
}

fn register_box_compose(engine: &mut Engine) {
    engine.register(
        "box.compose",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut action = String::new();
            let mut yaml = String::new();
            let mut target = "compose_result".to_string();
            let mut project_name = String::new();
            let mut file_name = "docker-compose.yml".to_string();

            if node.value.is_some() {
                action = resolve_node_value(_engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "action" => action = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "yaml" => yaml = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "file" | "file_name" => file_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = if project_name.is_empty() {
                Path::new(&data_dir).join("compose")
            } else {
                Path::new(&data_dir).join("compose").join(&project_name)
            };
            let compose_path = compose_dir.join(&file_name);

            let mut write_err = None;
            if !yaml.is_empty() {
                if let Err(e) = fs::create_dir_all(&compose_dir) {
                    write_err = Some(format!("Failed to create compose directory '{}': {}", compose_dir.display(), e));
                } else if let Err(e) = fs::write(&compose_path, yaml) {
                    write_err = Some(format!("Failed to write compose file '{}': {}", compose_path.display(), e));
                }
            }

            let res = if let Some(err) = write_err {
                Err(err)
            } else {
                let compose_path_str = compose_path.to_string_lossy();
                match action.as_str() {
                    "up" => compose_up(&compose_path_str),
                    "down" => compose_down(&compose_path_str),
                    "ps" => compose_ps(&compose_path_str),
                    "save" => Ok("Saved successfully".to_string()),
                    _ => Err(format!("Unknown compose action: {}", action)),
                }
            };

            let mut result = HashMap::new();
            match res {
                Ok(out) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String(out));
                    result.insert("exit_code".to_string(), Value::Int(0));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e));
                    result.insert("exit_code".to_string(), Value::Int(-1));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Run docker-compose commands natively in Rust".to_string(),
            example: "box.compose: 'up' { yaml: $yaml, as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_get_yaml(engine: &mut Engine) {
    engine.register(
        "box.compose_get_yaml",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            let mut file_name = "docker-compose.yml".to_string();
            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "file" | "file_name" => file_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = if project_name.is_empty() {
                Path::new(&data_dir).join("compose")
            } else {
                Path::new(&data_dir).join("compose").join(&project_name)
            };
            let compose_path = compose_dir.join(&file_name);
            let yaml = fs::read_to_string(compose_path).unwrap_or_default();

            let mut result = HashMap::new();
            result.insert("yaml".to_string(), Value::String(yaml));
            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Get compose YAML file content".to_string(),
            example: "box.compose_get_yaml { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_list_projects(engine: &mut Engine) {
    engine.register(
        "box.compose_list_projects",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose");
            
            let mut list = Vec::new();
            if let Ok(entries) = fs::read_dir(compose_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Ok(meta) = entry.metadata() {
                            if meta.is_dir() {
                                let mut map = HashMap::new();
                                map.insert("name".to_string(), Value::String(entry.file_name().to_string_lossy().into_owned()));
                                map.insert("is_dir".to_string(), Value::Bool(true));
                                list.push(Value::Map(map));
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List all compose projects".to_string(),
            example: "box.compose_list_projects { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_delete_project(engine: &mut Engine) {
    engine.register(
        "box.compose_delete_project",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if project_name.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let success = if compose_dir.exists() && compose_dir.is_dir() {
                fs::remove_dir_all(compose_dir).is_ok()
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Delete compose project".to_string(),
            example: "box.compose_delete_project { project_name: 'test', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn serde_json_to_zeno(val: &serde_json::Value) -> Value {
    match val {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => Value::List(arr.iter().map(serde_json_to_zeno).collect()),
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), serde_json_to_zeno(v));
            }
            Value::Map(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_parse_memory_bytes() {
        assert_eq!(parse_memory_bytes("512m"), 512 * 1024 * 1024);
        assert_eq!(parse_memory_bytes("1g"), 1 * 1024 * 1024 * 1024);
        assert_eq!(parse_memory_bytes("256k"), 256 * 1024);
        assert_eq!(parse_memory_bytes("1024b"), 1024);
        assert_eq!(parse_memory_bytes("50"), 50);
        assert_eq!(parse_memory_bytes(""), 0);
        assert_eq!(parse_memory_bytes("invalid"), 0);
    }

    #[test]
    fn test_compose_git_slots() {
        use zenocore::{Context, Scope};
        let _guard = TEST_MUTEX.lock().unwrap();
        
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_git_slots_{}", rand::random::<u32>()));
        fs::create_dir_all(&temp_dir).unwrap();
        
        let old_dir = std::env::var("ZENO_CONTAINER_DATA_DIR").ok();
        unsafe {
            std::env::set_var("ZENO_CONTAINER_DATA_DIR", &temp_dir);
        }

        let mut engine = Engine::new();
        register_box_compose_git_get(&mut engine);
        register_box_compose_git_save(&mut engine);
        
        let mut ctx = Context::new();
        let scope = Scope::new(None);
        
        // Save git settings
        let save_node = zenocore::parser::parse_string(
            r#"
            box.compose_git_save: {
                project_name: "test_git_project_name_123"
                repo_url: "https://github.com/user/repo.git"
                branch: "production"
                webhook_token: "secret123"
                as: $success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &save_node, &scope).unwrap();
        assert_eq!(scope.get("success"), Some(Value::Bool(true)));
        
        // Get git settings
        let get_node = zenocore::parser::parse_string(
            r#"
            box.compose_git_get: {
                project_name: "test_git_project_name_123"
                as: $git
            }
            "#,
            "test"
        ).unwrap();
        let scope2 = Scope::new(None);
        engine.execute(&mut ctx, &get_node, &scope2).unwrap();
        let git_val = scope2.get("git").unwrap();
        assert!(matches!(git_val, Value::Map(_)));
        if let Value::Map(ref map) = git_val {
            assert_eq!(map.get("repo_url").unwrap(), &Value::String("https://github.com/user/repo.git".to_string()));
            assert_eq!(map.get("branch").unwrap(), &Value::String("production".to_string()));
            assert_eq!(map.get("webhook_token").unwrap(), &Value::String("secret123".to_string()));
        }
        
        let _ = fs::remove_dir_all(&temp_dir);
        unsafe {
            if let Some(d) = old_dir {
                std::env::set_var("ZENO_CONTAINER_DATA_DIR", d);
            } else {
                std::env::remove_var("ZENO_CONTAINER_DATA_DIR");
            }
        }
    }

    #[test]
    fn test_yaml_deserialization_and_order() {
        let yaml_content = r#"
version: '3.8'
services:
  web:
    image: nginx:latest
    depends_on:
      - app
  app:
    image: my-node-app:latest
    depends_on:
      - db
  db:
    image: postgres:latest
"#;
        let cf: ComposeFile = serde_yaml::from_str(yaml_content).expect("Failed to parse mock YAML");
        assert!(cf.services.contains_key("web"));
        assert!(cf.services.contains_key("app"));
        assert!(cf.services.contains_key("db"));

        let ordered = order_services(&cf.services);
        assert_eq!(ordered.len(), 3);
        // db has no dependencies, app depends on db, web depends on app
        // Therefore, order should be db first, then app, then web
        assert_eq!(ordered[0], "db");
        assert_eq!(ordered[1], "app");
        assert_eq!(ordered[2], "web");
    }

    #[test]
    fn test_parse_port_rule() {
        // Test parsing simple ports
        let r1 = parse_port_rule("80").unwrap();
        assert_eq!(r1.host_ip, None);
        assert_eq!(r1.host_port, "80");
        assert_eq!(r1.container_port, "80");
        assert_eq!(r1.protocol, "tcp");

        // Test host:container ports
        let r2 = parse_port_rule("8080:80").unwrap();
        assert_eq!(r2.host_ip, None);
        assert_eq!(r2.host_port, "8080");
        assert_eq!(r2.container_port, "80");
        assert_eq!(r2.protocol, "tcp");

        // Test IP:host:container ports
        let r3 = parse_port_rule("127.0.0.1:8080:80").unwrap();
        assert_eq!(r3.host_ip, Some("127.0.0.1".to_string()));
        assert_eq!(r3.host_port, "8080");
        assert_eq!(r3.container_port, "80");
        assert_eq!(r3.protocol, "tcp");

        // Test with protocol suffix
        let r4 = parse_port_rule("127.0.0.1:8080:80/udp").unwrap();
        assert_eq!(r4.host_ip, Some("127.0.0.1".to_string()));
        assert_eq!(r4.host_port, "8080");
        assert_eq!(r4.container_port, "80");
        assert_eq!(r4.protocol, "udp");

        // Test ranges
        let r5 = parse_port_rule("80-82:80-82").unwrap();
        assert_eq!(r5.host_ip, None);
        assert_eq!(r5.host_port, "80-82");
        assert_eq!(r5.container_port, "80-82");
        assert_eq!(r5.protocol, "tcp");
    }

    #[test]
    fn test_compose_yaml_deserialization_advanced() {
        let yaml_content = r#"
version: '3.8'
services:
  web:
    image: nginx:latest
    entrypoint: /usr/bin/nginx
    command: ["-g", "daemon off;"]
    ports:
      - 80
      - "8080:80"
      - target: 9000
        published: 9001
        host_ip: 127.0.0.1
        protocol: udp
    environment:
      DEBUG: true
      PORT: 8080
      DB_HOST: "postgres"
    env_file:
      - .env
    extra_hosts:
      somehost: "10.0.0.1"
      otherhost: "10.0.0.2"
volumes:
  db_data: {}
"#;
        let cf: ComposeFile = serde_yaml::from_str(yaml_content).expect("Failed to parse advanced mock YAML");
        let web = cf.services.get("web").unwrap();
        
        // Assert entrypoint and command
        assert_eq!(web.entrypoint.as_ref().unwrap().0, vec!["/usr/bin/nginx".to_string()]);
        assert_eq!(web.command.as_ref().unwrap().0, vec!["-g".to_string(), "daemon off;".to_string()]);

        // Assert ports mapping
        let ports = &web.ports.as_ref().unwrap().0;
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0], "80");
        assert_eq!(ports[1], "8080:80");
        assert_eq!(ports[2], "127.0.0.1:9001:9000/udp");

        // Assert environment variables coercion
        let env = &web.environment.as_ref().unwrap().0;
        assert_eq!(env.get("DEBUG").unwrap(), "true");
        assert_eq!(env.get("PORT").unwrap(), "8080");
        assert_eq!(env.get("DB_HOST").unwrap(), "postgres");

        // Assert env_file
        let env_file = web.env_file.as_ref().unwrap();
        assert_eq!(env_file.as_sequence().unwrap()[0].as_str().unwrap(), ".env");

        // Assert extra_hosts
        let extra_hosts = &web.extra_hosts.as_ref().unwrap().0;
        assert!(extra_hosts.contains(&"somehost:10.0.0.1".to_string()));
        assert!(extra_hosts.contains(&"otherhost:10.0.0.2".to_string()));

        // Assert top-level volumes
        assert!(cf.volumes.as_ref().unwrap().contains_key("db_data"));
    }

    #[test]
    fn test_docker_config_and_registry_auth() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_docker_config_{}", rand::random::<u32>()));
        let docker_dir = temp_dir.join(".docker");
        fs::create_dir_all(&docker_dir).unwrap();
        
        let config_content = r#"{
            "auths": {
                "ghcr.io": {
                    "auth": "bXktdXNlcjpteS1wYXNzd29yZA=="
                },
                "https://index.docker.io/v1/": {
                    "auth": "ZG9ja2VyLXVzZXI6ZG9ja2VyLXBhc3N3b3Jk"
                }
            }
        }"#;
        fs::write(docker_dir.join("config.json"), config_content).unwrap();

        // Backup current HOME and override it
        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &temp_dir);
        }

        // Test matching a custom registry
        let ghcr_auth = get_docker_auth_for_registry("https://ghcr.io");
        assert_eq!(ghcr_auth, Some(("my-user".to_string(), "my-password".to_string())));

        // Test matching docker hub index
        let docker_auth = get_docker_auth_for_registry("https://registry-1.docker.io");
        assert_eq!(docker_auth, Some(("docker-user".to_string(), "docker-password".to_string())));

        // Clean up HOME env var and remove mock files
        unsafe {
            if let Some(h) = old_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_www_authenticate() {
        let header = r#"Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:user/repo:pull""#;
        let parsed = parse_www_authenticate(header);
        assert_eq!(
            parsed,
            Some((
                "https://ghcr.io/token".to_string(),
                "ghcr.io".to_string(),
                "repository:user/repo:pull".to_string()
            ))
        );
    }

    #[test]
    fn test_registry_slots_engine() {
        use zenocore::{Context, Scope};
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_registry_slots_{}", rand::random::<u32>()));
        let docker_dir = temp_dir.join(".docker");
        fs::create_dir_all(&docker_dir).unwrap();

        // Backup current HOME and override it
        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &temp_dir);
        }

        let mut engine = Engine::new();
        register_box_registry_list(&mut engine);
        register_box_registry_add(&mut engine);
        register_box_registry_delete(&mut engine);

        let mut ctx = Context::new();
        let scope = Scope::new(None);

        // 1. Add registry ghcr.io
        let node_add = zenocore::parser::parse_string(
            r#"
            box.registry_add: {
                registry: 'ghcr.io'
                username: 'test-user'
                password: 'test-password'
                as: $success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_add, &scope).unwrap();
        assert_eq!(scope.get("success"), Some(Value::Bool(true)));

        // 2. List registries
        let node_list = zenocore::parser::parse_string(
            r#"
            box.registry_list: {
                as: $list
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_list, &scope).unwrap();
        
        if let Some(Value::List(lst)) = scope.get("list") {
            assert_eq!(lst.len(), 1);
            assert_eq!(lst[0], Value::String("ghcr.io".to_string()));
        } else {
            panic!("Expected list value");
        }

        // 3. Delete registry
        let node_del = zenocore::parser::parse_string(
            r#"
            box.registry_delete: {
                registry: 'ghcr.io'
                as: $del_success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_del, &scope).unwrap();
        assert_eq!(scope.get("del_success"), Some(Value::Bool(true)));

        // 4. Verify list is empty
        engine.execute(&mut ctx, &node_list, &scope).unwrap();
        if let Some(Value::List(lst)) = scope.get("list") {
            assert_eq!(lst.len(), 0);
        } else {
            panic!("Expected list value");
        }

        // Clean up HOME env var and remove mock files
        unsafe {
            if let Some(h) = old_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

fn register_box_registry_list(engine: &mut Engine) {
    engine.register(
        "box.registry_list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let mut list = Vec::new();
            if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
                let p = home.join(".docker/config.json");
                if p.exists() {
                    if let Ok(content) = fs::read_to_string(&p) {
                        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(auths) = config.get("auths").and_then(|a| a.as_object()) {
                                for (key, _) in auths {
                                    list.push(Value::String(key.clone()));
                                }
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List Docker Registry logins".to_string(),
            example: "box.registry_list { as: $logins }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_registry_add(engine: &mut Engine) {
    engine.register(
        "box.registry_add",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut registry = String::new();
            let mut username = String::new();
            let mut password = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "registry" => registry = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "username" => username = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "password" => password = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if registry.is_empty() || username.is_empty() || password.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let home = match std::env::var("HOME").ok().map(PathBuf::from) {
                Some(h) => h,
                None => {
                    scope.set(&target, Value::Bool(false));
                    return Ok(());
                }
            };

            let docker_dir = home.join(".docker");
            let _ = fs::create_dir_all(&docker_dir);
            let p = docker_dir.join("config.json");

            let mut config = if p.exists() {
                fs::read_to_string(&p)
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .unwrap_or_else(|| serde_json::json!({ "auths": {} }))
            } else {
                serde_json::json!({ "auths": {} })
            };

            if config.get("auths").is_none() {
                if let Some(obj) = config.as_object_mut() {
                    obj.insert("auths".to_string(), serde_json::json!({}));
                }
            }

            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);

            if let Some(auths) = config.get_mut("auths").and_then(|a| a.as_object_mut()) {
                auths.insert(
                    registry.clone(),
                    serde_json::json!({
                        "auth": encoded
                    }),
                );
            }

            let success = if let Ok(pretty) = serde_json::to_string_pretty(&config) {
                fs::write(p, pretty).is_ok()
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Add Docker Registry credentials".to_string(),
            example: "box.registry_add { registry: 'ghcr.io', username: 'foo', password: 'bar', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_registry_delete(engine: &mut Engine) {
    engine.register(
        "box.registry_delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut registry = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "registry" => registry = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if registry.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let home = match std::env::var("HOME").ok().map(PathBuf::from) {
                Some(h) => h,
                None => {
                    scope.set(&target, Value::Bool(false));
                    return Ok(());
                }
            };

            let p = home.join(".docker/config.json");
            if !p.exists() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let mut success = false;
            if let Ok(content) = fs::read_to_string(&p) {
                if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(auths) = config.get_mut("auths").and_then(|a| a.as_object_mut()) {
                        if auths.remove(&registry).is_some() {
                            if let Ok(pretty) = serde_json::to_string_pretty(&config) {
                                success = fs::write(p, pretty).is_ok();
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Delete Docker Registry credentials".to_string(),
            example: "box.registry_delete { registry: 'ghcr.io', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_get(engine: &mut Engine) {
    engine.register(
        "box.compose_git_get",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let git_config_path = Path::new(&data_dir).join("compose").join(&project_name).join(".zeno-git.json");

            let mut result = HashMap::new();
            if git_config_path.exists() {
                if let Ok(content) = fs::read_to_string(&git_config_path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(obj) = val.as_object() {
                            for (k, v) in obj {
                                result.insert(k.clone(), Value::String(v.as_str().unwrap_or("").to_string()));
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Get Git settings for a Compose project".to_string(),
            example: "box.compose_git_get { project_name: 'my-app', as: $git }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_save(engine: &mut Engine) {
    engine.register(
        "box.compose_git_save",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            let mut repo_url = String::new();
            let mut branch = "main".to_string();
            let mut webhook_token = String::new();

            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "repo_url" => repo_url = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "branch" => branch = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "webhook_token" => webhook_token = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let git_config_path = compose_dir.join(".zeno-git.json");

            let mut success = false;
            if fs::create_dir_all(&compose_dir).is_ok() {
                let config = serde_json::json!({
                    "repo_url": repo_url,
                    "branch": branch,
                    "webhook_token": webhook_token
                });
                if let Ok(pretty) = serde_json::to_string_pretty(&config) {
                    success = fs::write(&git_config_path, pretty).is_ok();
                }
            }

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Save Git settings for a Compose project".to_string(),
            example: "box.compose_git_save { project_name: 'my-app', repo_url: 'https://...', branch: 'main', webhook_token: '...', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_sync(engine: &mut Engine) {
    engine.register(
        "box.compose_git_sync",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let git_config_path = compose_dir.join(".zeno-git.json");

            let mut result = HashMap::new();
            result.insert("success".to_string(), Value::Bool(false));

            if !git_config_path.exists() {
                result.insert("stderr".to_string(), Value::String("No Git configuration found for this project".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let config_content = match fs::read_to_string(&git_config_path) {
                Ok(c) => c,
                Err(e) => {
                    result.insert("stderr".to_string(), Value::String(format!("Failed to read Git config: {}", e)));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            };

            let config: serde_json::Value = match serde_json::from_str(&config_content) {
                Ok(v) => v,
                Err(e) => {
                    result.insert("stderr".to_string(), Value::String(format!("Failed to parse Git config: {}", e)));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            };

            let repo_url = config.get("repo_url").and_then(|v| v.as_str()).unwrap_or("");
            let branch = config.get("branch").and_then(|v| v.as_str()).unwrap_or("main");

            if repo_url.is_empty() {
                result.insert("stderr".to_string(), Value::String("Git repository URL is empty".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let volume_path = Path::new(&data_dir).join("volumes").join(format!("{}_app_data", project_name));
            if !volume_path.exists() {
                let _ = fs::create_dir_all(&volume_path);
            }

            let git_dir = volume_path.join(".git");
            let git_res = if git_dir.exists() {
                let fetch_out = std::process::Command::new("git")
                    .arg("fetch")
                    .arg("--all")
                    .current_dir(&volume_path)
                    .output();

                match fetch_out {
                    Ok(out) if out.status.success() => {
                        let reset_out = std::process::Command::new("git")
                            .arg("reset")
                            .arg("--hard")
                            .arg(format!("origin/{}", branch))
                            .current_dir(&volume_path)
                            .output();

                        match reset_out {
                            Ok(out2) if out2.status.success() => Ok("Git pull & reset --hard completed successfully".to_string()),
                            Ok(out2) => Err(format!("git reset failed: {}", String::from_utf8_lossy(&out2.stderr))),
                            Err(e) => Err(format!("Failed to run git reset: {}", e)),
                        }
                    }
                    Ok(out) => Err(format!("git fetch failed: {}", String::from_utf8_lossy(&out.stderr))),
                    Err(e) => Err(format!("Failed to run git fetch: {}", e)),
                }
            } else {
                let run_init = std::process::Command::new("git").arg("init").current_dir(&volume_path).status();
                let run_remote = std::process::Command::new("git").arg("remote").arg("add").arg("origin").arg(repo_url).current_dir(&volume_path).status();
                let run_fetch = std::process::Command::new("git").arg("fetch").arg("origin").current_dir(&volume_path).status();
                let run_checkout = std::process::Command::new("git").arg("checkout").arg("-f").arg(branch).current_dir(&volume_path).status();

                match (run_init, run_remote, run_fetch, run_checkout) {
                    (Ok(s1), Ok(s2), Ok(s3), Ok(s4)) if s1.success() && s2.success() && s3.success() && s4.success() => {
                        Ok("Git clone/checkout completed successfully".to_string())
                    }
                    _ => Err("Git initialization or checkout failed. Please verify the repository URL and permissions.".to_string()),
                }
            };

            match git_res {
                Ok(stdout_msg) => {
                    let compose_path = compose_dir.join("docker-compose.yml");
                    if compose_path.exists() {
                        match compose_up(&compose_path.to_string_lossy()) {
                            Ok(up_msg) => {
                                result.insert("success".to_string(), Value::Bool(true));
                                result.insert("stdout".to_string(), Value::String(format!("{}\n\n{}", stdout_msg, up_msg)));
                            }
                            Err(e) => {
                                result.insert("stderr".to_string(), Value::String(format!("Git pulled but compose restart failed: {}", e)));
                            }
                        }
                    } else {
                        result.insert("success".to_string(), Value::Bool(true));
                        result.insert("stdout".to_string(), Value::String(format!("{} (Note: No docker-compose.yml to restart)", stdout_msg)));
                    }
                }
                Err(e) => {
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Synchronize/pull Git repository files to project volume and restart compose containers".to_string(),
            example: "box.compose_git_sync { project_name: 'my-app', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}



