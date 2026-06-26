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
}

fn get_runc_bin() -> String {
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

async fn get_registry_token(client: &reqwest::Client, img: &ImageRef) -> Result<String, String> {
    let host = img.registry.trim_start_matches("https://").trim_start_matches("http://");
    if host == "registry-1.docker.io" {
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull",
            img.repository
        );
        let resp = client.get(&auth_url).send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(tok) = json.get("token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
            if let Some(tok) = json.get("access_token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
        }
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
    if !token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
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
        if !token.is_empty() {
            req2 = req2.header("Authorization", format!("Bearer {}", token));
        }
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
    if !token.is_empty() {
        req_cfg = req_cfg.header("Authorization", format!("Bearer {}", token));
    }
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
                if !token.is_empty() {
                    req_blob = req_blob.header("Authorization", format!("Bearer {}", token));
                }
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

    Ok(final_cmd)
}

fn get_data_dir() -> String {
    std::env::var("ZENO_CONTAINER_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string())
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

    // Use sudo so the mount command succeeds regardless of whether the zeno
    // process itself was started with root privileges.
    let status = Command::new("sudo")
        .args(&["mount", "-t", "overlay", "overlay", "-o", &opts, &dst_rootfs_str])
        .status()
        .map_err(|e| format!("Failed to run mount command: {}", e))?;

    if !status.success() {
        return Err(format!("mount command failed with status: {:?}", status.code()));
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

    for m in mounts {
        let parts: Vec<&str> = m.splitn(2, ':').collect();
        if parts.len() == 2 {
            let host_path = parts[0];
            let container_path = parts[1];
            let abs_host = fs::canonicalize(host_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| host_path.to_string());

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
                    "bounding": ["CAP_NET_BIND_SERVICE", "CAP_KILL"],
                    "effective": ["CAP_NET_BIND_SERVICE", "CAP_KILL"],
                    "inheritable": ["CAP_NET_BIND_SERVICE", "CAP_KILL"],
                    "permitted": ["CAP_NET_BIND_SERVICE", "CAP_KILL"]
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

    generate_config_json(
        &bundle_p,
        cmd.clone(),
        env.clone(),
        cwd,
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
        env: Some(env),
        mounts: Some(mounts),
        cwd: Some(cwd.to_string()),
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
    let output = Command::new("ip").args(&["link", "show", "zenobr0"]).output();
    if output.is_ok() && output.unwrap().status.success() {
        return Ok(());
    }

    let _ = Command::new("ip").args(&["link", "add", "name", "zenobr0", "type", "bridge"]).status();
    let _ = Command::new("ip").args(&["addr", "add", "172.20.0.1/16", "dev", "zenobr0"]).status();
    let _ = Command::new("ip").args(&["link", "set", "zenobr0", "up"]).status();

    let _ = Command::new("iptables")
        .args(&["-t", "nat", "-C", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"])
        .status();
    let _ = Command::new("iptables")
        .args(&["-t", "nat", "-A", "POSTROUTING", "-s", "172.20.0.0/16", "!", "-o", "zenobr0", "-j", "MASQUERADE"])
        .status();

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

    if let Ok(containers) = container_list_internal(data_dir) {
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

    let mut veth_host = format!("veth-h-{}", container_id);
    let mut veth_guest = format!("veth-g-{}", container_id);
    if veth_host.len() > 15 { veth_host.truncate(15); }
    if veth_guest.len() > 15 { veth_guest.truncate(15); }

    let _ = Command::new("ip").args(&["link", "delete", &veth_host]).status();

    let status = Command::new("ip")
        .args(&["link", "add", &veth_host, "type", "veth", "peer", "name", &veth_guest])
        .status()
        .map_err(|e| format!("Failed to create veth pair: {}", e))?;
    if !status.success() {
        return Err("Failed to create veth pair".to_string());
    }

    let _ = Command::new("ip").args(&["link", "set", &veth_host, "master", &bridge_id]).status();
    let _ = Command::new("ip").args(&["link", "set", &veth_host, "up"]).status();

    let pid_str = pid.to_string();
    let status = Command::new("ip")
        .args(&["link", "set", &veth_guest, "netns", &pid_str])
        .status()
        .map_err(|e| format!("Failed to move guest veth: {}", e))?;
    if !status.success() {
        return Err("Failed to move guest interface to container netns".to_string());
    }

    let _ = Command::new("nsenter").args(&["-t", &pid_str, "-n", "ip", "link", "set", &veth_guest, "name", "eth0"]).status();
    let _ = Command::new("nsenter").args(&["-t", &pid_str, "-n", "ip", "addr", "add", &format!("{}/16", ip), "dev", "eth0"]).status();
    let _ = Command::new("nsenter").args(&["-t", &pid_str, "-n", "ip", "link", "set", "eth0", "up"]).status();
    let _ = Command::new("nsenter").args(&["-t", &pid_str, "-n", "ip", "route", "add", "default", "via", &gateway_ip]).status();

    let resolv_path = rootfs_dir(data_dir, container_id).join("etc/resolv.conf");
    let _ = fs::write(resolv_path, "nameserver 8.8.8.8\nnameserver 1.1.1.1\n");

    for p in ports {
        let parts: Vec<&str> = p.split(':').collect();
        if parts.len() == 2 {
            let host_port = parts[0];
            let container_port = parts[1];
            let _ = Command::new("iptables").args(&["-t", "nat", "-A", "PREROUTING", "-p", "tcp", "--dport", host_port, "-j", "DNAT", "--to-destination", &format!("{}:{}", ip, container_port)]).status();
            let _ = Command::new("iptables").args(&["-t", "nat", "-A", "OUTPUT", "-p", "tcp", "--dport", host_port, "-j", "DNAT", "--to-destination", &format!("{}:{}", ip, container_port)]).status();
        }
    }

    Ok(ip)
}

fn clean_container_network(container_id: &str, ip: &str, ports: &[String]) {
    for p in ports {
        let parts: Vec<&str> = p.split(':').collect();
        if parts.len() == 2 {
            let host_port = parts[0];
            let container_port = parts[1];
            let _ = Command::new("iptables").args(&["-t", "nat", "-D", "PREROUTING", "-p", "tcp", "--dport", host_port, "-j", "DNAT", "--to-destination", &format!("{}:{}", ip, container_port)]).status();
            let _ = Command::new("iptables").args(&["-t", "nat", "-D", "OUTPUT", "-p", "tcp", "--dport", host_port, "-j", "DNAT", "--to-destination", &format!("{}:{}", ip, container_port)]).status();
        }
    }
    let mut veth_host = format!("veth-h-{}", container_id);
    if veth_host.len() > 15 { veth_host.truncate(15); }
    let _ = Command::new("ip").args(&["link", "delete", &veth_host]).status();
}

fn sync_hosts_entries(data_dir: &str) -> Result<(), String> {
    let containers = container_list_internal(data_dir)?;
    
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

pub(crate) fn container_list_internal(data_dir: &str) -> Result<Vec<ContainerState>, String> {
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
                                        let _ = save_container_state(&state);
                                    }
                                }
                            } else {
                                if state.status == "running" || state.status == "created" {
                                    state.status = "stopped".to_string();
                                    state.pid = 0;
                                    let _ = save_container_state(&state);
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

    let bundle_p = bundle_dir(&data_dir, id);

    let _ = runc_exec(&["delete", "--force", id]);

    let run_create = runc_exec(&["create", "-b", &bundle_p.to_string_lossy(), id])
        .map_err(|e| format!("runc create process failed: {}", e))?;
    if !run_create.status.success() {
        state.status = "failed".to_string();
        let _ = save_container_state(&state);
        let err_msg = String::from_utf8_lossy(&run_create.stderr);
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
        let _ = Command::new("sudo").args(&["umount", "-l", &dst_rootfs.to_string_lossy().to_string()]).status();
    }

    let cont_p = container_dir(&data_dir, id);
    let _ = fs::remove_dir_all(cont_p);

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

    let _ = Command::new("ip").args(&["link", "add", "name", &bridge_id, "type", "bridge"]).status();
    let _ = Command::new("ip").args(&["addr", "add", &format!("{}/16", gateway), "dev", &bridge_id]).status();
    let _ = Command::new("ip").args(&["link", "set", &bridge_id, "up"]).status();

    let _ = Command::new("iptables").args(&["-t", "nat", "-A", "POSTROUTING", "-s", &subnet, "!", "-o", &bridge_id, "-j", "MASQUERADE"]).status();

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

    let containers = container_list_internal(data_dir)?;
    for c in containers {
        if c.network.as_ref().map(|s| s == name || s == &net.id).unwrap_or(false) && c.status == "running" {
            return Err(format!("Network is in use by running container {}", c.id));
        }
    }

    let _ = Command::new("ip").args(&["link", "set", &net.id, "down"]).status();
    let _ = Command::new("ip").args(&["link", "delete", &net.id]).status();
    let _ = Command::new("iptables").args(&["-t", "nat", "-D", "POSTROUTING", "-s", &net.subnet, "!", "-o", &net.id, "-j", "MASQUERADE"]).status();

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
            let mut cmd = String::new();
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
                        "cmd" => cmd = resolved.to_string_coerce(),
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

            let cmd_vec = if cmd.is_empty() { Vec::new() } else {
                cmd.split_whitespace().map(|s| s.to_string()).collect()
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
            if let Ok(containers) = container_list_internal(&data_dir) {
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
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    env.insert(k, v);
                }
                Ok(ComposeEnvironment(env))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut env = HashMap::new();
                while let Some(item) = seq.next_element::<String>()? {
                    let parts: Vec<&str> = item.splitn(2, '=').collect();
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
                formatter.write_str("a sequence of strings or integers")
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
    pub volumes: Option<Vec<String>>,
    pub command: Option<String>,
    pub depends_on: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthCheck>,
    pub mem_limit: Option<String>,
    pub cpus: Option<f64>,
    pub oom_score_adj: Option<i32>,
    pub read_only: Option<bool>,
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

fn inject_hosts_entries(data_dir: &str, container_id: &str, services: &HashMap<String, ComposeService>, current_name: &str) -> Result<(), String> {
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
        if !cache_dir.exists() {
            output.push_str(&format!("  ▶ Image {} not found locally. Pulling...\n", image));
            let rt = tokio::runtime::Handle::current();
            let pull_res = tokio::task::block_in_place(|| {
                rt.block_on(async { pull_image_rust(image).await })
            });
            if let Err(e) = pull_res {
                return Err(format!("Failed to pull image {}: {}", image, e));
            }
        }

        let container_name = svc.container_name.as_ref().unwrap_or(&name);

        let cont_p = container_dir(&data_dir, container_name);
        if cont_p.exists() {
            output.push_str(&format!("  ▶ Container '{}' already exists. Stopping and removing first...\n", container_name));
            let _ = container_stop(container_name);
            let _ = container_delete(container_name);
        }

        let cmd_args = if let Some(ref command) = svc.command {
            command.split_whitespace().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };

        let env = if let Some(ref e) = svc.environment {
            e.0.clone()
        } else {
            HashMap::new()
        };

        let volumes = svc.volumes.clone().unwrap_or_default();
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

    let containers = container_list_internal(&data_dir)?;
    
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

            if node.value.is_some() {
                action = resolve_node_value(_engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "action" => action = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "yaml" => yaml = resolve_node_value(_engine, child, scope).to_string_coerce(),
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
            let compose_dir = if project_name.is_empty() {
                Path::new(&data_dir).join("compose")
            } else {
                Path::new(&data_dir).join("compose").join(&project_name)
            };
            let compose_path = compose_dir.join("docker-compose.yml");

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
            let compose_dir = if project_name.is_empty() {
                Path::new(&data_dir).join("compose")
            } else {
                Path::new(&data_dir).join("compose").join(&project_name)
            };
            let compose_path = compose_dir.join("docker-compose.yml");
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
}



