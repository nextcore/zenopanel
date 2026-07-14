use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write};
use std::process::Command;
use serde::{Serialize, Deserialize};

pub const DEFAULT_DATA_DIR: &str = "/var/lib/zeno-container";

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

pub struct ImageRef {
    pub registry: String,
    pub repository: String,
    pub tag: String,
}

pub fn parse_image_ref(image: &str) -> ImageRef {
    let (image_no_tag, tag) = if let Some(idx) = image.rfind(':') {
        let potential_tag = &image[idx + 1..];
        if potential_tag.contains('/') {
            (image, "latest")
        } else {
            (&image[..idx], potential_tag)
        }
    } else {
        (image, "latest")
    };

    let parts: Vec<&str> = image_no_tag.splitn(2, '/').collect();
    let (registry, repository) = if parts.len() == 2 && (parts[0].contains('.') || parts[0].contains(':')) {
        (parts[0], parts[1].to_string())
    } else if parts.len() == 1 {
        ("https://registry-1.docker.io", format!("library/{}", parts[0]))
    } else {
        ("https://registry-1.docker.io", image_no_tag.to_string())
    };

    let final_registry = if registry.starts_with("http://") || registry.starts_with("https://") {
        registry.to_string()
    } else {
        format!("https://{}", registry)
    };

    ImageRef {
        registry: final_registry,
        repository,
        tag: tag.to_string(),
    }
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

pub fn get_runc_bin() -> String {
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
        const RUNC_BYTES: &[u8] = include_bytes!("../runc-linux-amd64");
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

pub(crate) fn runc_exec(args: &[&str]) -> io::Result<std::process::Output> {
    let runc_bin = get_runc_bin();
    let root = format!("{}/runc", get_data_dir());
    let mut all_args = vec!["--root", &root];
    all_args.extend_from_slice(args);
    Command::new(&runc_bin).args(&all_args).output()
}

pub(crate) fn get_data_dir() -> String {
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

pub(crate) fn container_dir(data_dir: &str, id: &str) -> PathBuf {
    Path::new(data_dir).join("containers").join(id)
}

pub(crate) fn bundle_dir(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("bundle")
}

pub(crate) fn rootfs_dir(data_dir: &str, id: &str) -> PathBuf {
    bundle_dir(data_dir, id).join("rootfs")
}

pub(crate) fn state_file(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("state.json")
}

pub(crate) fn log_path(data_dir: &str, id: &str) -> PathBuf {
    container_dir(data_dir, id).join("console.log")
}

pub(crate) fn is_overlay_mounted(mount_point: &str) -> bool {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    mounts.lines().any(|line| {
        let parts: Vec<&str> = line.split_whitespace().collect();
        parts.len() >= 3 && parts[2] == "overlay" && parts[1] == mount_point
    })
}

pub(crate) fn run_privileged_status(cmd: &str, args: &[&str]) -> io::Result<std::process::ExitStatus> {
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

pub(crate) fn run_privileged_output(cmd: &str, args: &[&str]) -> io::Result<std::process::Output> {
    let is_root = unsafe { libc::getuid() == 0 };
    if is_root {
        Command::new(cmd)
            .args(args)
            .output()
    } else {
        let mut all_args = vec![cmd];
        all_args.extend_from_slice(args);
        Command::new("sudo")
            .args(&all_args)
            .output()
    }
}

pub(crate) fn run_cmd_status_silent(cmd: &str, args: &[&str]) -> io::Result<std::process::ExitStatus> {
    Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
}

pub(crate) fn save_container_state(state: &ContainerState) -> Result<(), String> {
    let data_dir = get_data_dir();
    let p = state_file(&data_dir, &state.id);
    let f = File::create(p).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(f, state).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn load_container_state(id: &str) -> Result<ContainerState, String> {
    let data_dir = get_data_dir();
    let p = state_file(&data_dir, id);
    let f = File::open(p).map_err(|e| e.to_string())?;
    let state: ContainerState = serde_json::from_reader(f).map_err(|e| e.to_string())?;
    Ok(state)
}

pub(crate) fn get_networks(data_dir: &str) -> Vec<NetworkConfig> {
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

pub(crate) fn save_networks(data_dir: &str, nets: &[NetworkConfig]) -> Result<(), String> {
    let path = Path::new(data_dir).join("networks.json");
    let f = File::create(path).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(f, nets).map_err(|e| e.to_string())?;
    Ok(())
}
