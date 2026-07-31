use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use serde_json::json;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

use super::common::{
    get_data_dir, container_dir, bundle_dir, rootfs_dir, state_file, log_path,
    runc_exec, run_privileged_status,
    save_container_state, load_container_state,
    ContainerState, parse_image_ref
};
use super::image::{mount_overlayfs, get_image_default_cmd};
use super::network::{configure_container_network, clean_container_network, sync_hosts_entries};

pub fn register(engine: &mut Engine) {
    register_box_create(engine);
    register_box_start(engine);
    register_box_stop(engine);
    register_box_delete(engine);
    register_box_list(engine);
    register_box_inspect(engine);
    register_box_logs(engine);
    register_box_rootfs_path(engine);
    register_box_update(engine);
    register_box_import_docker(engine);
    register_system_list_docker_containers(engine);
}

fn check_oom_killed(id: &str) -> bool {
    let candidates = [
        format!("/sys/fs/cgroup/runc/{}/memory.events", id),
        format!("/sys/fs/cgroup/{}/memory.events", id),
        format!("/sys/fs/cgroup/system.slice/runc-{}.scope/memory.events", id),
        format!("/sys/fs/cgroup/unified/runc/{}/memory.events", id),
    ];
    for path in &candidates {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if line.starts_with("oom_kill ") {
                    if let Some(val_str) = line.split_whitespace().nth(1) {
                        if let Ok(val) = val_str.parse::<i32>() {
                            if val > 0 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn container_list_internal(data_dir: &str, auto_restart: bool) -> Result<Vec<ContainerState>, String> {
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
                    if state.status != "stopped" && state.status != "oom_killed" {
                        let output = runc_exec(&["state", &id]);
                        if let Ok(out) = output {
                            if runc_exec(&["state", &id]).is_ok() && out.status.success() {
                                let out_str = String::from_utf8_lossy(&out.stdout);
                                if let Ok(runc_st) = serde_json::from_str::<serde_json::Value>(&out_str) {
                                    let mut runc_status = runc_st.get("status").and_then(|s| s.as_str()).unwrap_or("stopped").to_string();
                                    let runc_pid = runc_st.get("pid").and_then(|p| p.as_i64()).unwrap_or(0) as i32;

                                    if runc_status == "stopped" && check_oom_killed(&id) {
                                        runc_status = "oom_killed".to_string();
                                    }

                                    if state.status != runc_status || state.pid != runc_pid {
                                        state.status = runc_status;
                                        state.pid = runc_pid;
                                        if let Err(e) = save_container_state(&state) {
                                            eprintln!("  ⚠ Failed to save container state: {}", e);
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                if state.status == "running" || state.status == "created" {
                                    let is_oom = check_oom_killed(&id);
                                    state.status = if is_oom { "oom_killed".to_string() } else { "stopped".to_string() };
                                    state.pid = 0;
                                    if let Err(e) = save_container_state(&state) {
                                        eprintln!("  ⚠ Failed to save container state: {}", e);
                                        continue;
                                    }
                                }
                            }
                        }
                    }

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

pub(crate) fn container_create(
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

pub(crate) fn container_start(id: &str) -> Result<(), String> {
    let data_dir = get_data_dir();
    let mut state = load_container_state(id)?;
    if state.status == "running" {
        return Err(format!("Container {} is already running", id));
    }

    mount_overlayfs(&state.image, &data_dir, id)?;

    let old_ip = state.env.as_ref().and_then(|e| e.get("ZENO_IP").cloned()).unwrap_or_default();
    let old_ports = state.ports.clone().unwrap_or_default();
    clean_container_network(id, &old_ip, &old_ports);

    let bundle_p = bundle_dir(&data_dir, id);

    let _ = runc_exec(&["delete", "--force", id]);

    let log_p = log_path(&data_dir, id);
    let log_file = File::create(&log_p).map_err(|e| format!("Failed to create log file: {}", e))?;

    let runc_bin = crate::slots::zeno_box::get_runc_bin();
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

pub(crate) fn container_stop(id: &str) -> Result<(), String> {
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

pub(crate) fn container_delete(id: &str) -> Result<(), String> {
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
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let cont_p = container_dir(&data_dir, id);
    
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

pub(crate) fn container_update(id: &str, memory_limit: i64, cpu_limit: f64) -> Result<(), String> {
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

fn register_system_list_docker_containers(engine: &mut Engine) {
    engine.register(
        "system.list_docker_containers",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "containers".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let output = Command::new("docker")
                .args(&["ps", "-a", "--format", "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Ports}}"])
                .output();

            let mut list = Vec::new();
            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        let parts: Vec<&str> = line.split('|').collect();
                        if parts.len() >= 4 {
                            let mut item = HashMap::new();
                            item.insert("id".to_string(), Value::String(parts[0].to_string()));
                            item.insert("name".to_string(), Value::String(parts[1].to_string()));
                            item.insert("image".to_string(), Value::String(parts[2].to_string()));
                            item.insert("status".to_string(), Value::String(parts[3].to_string()));
                            item.insert("ports".to_string(), Value::String(if parts.len() > 4 { parts[4].to_string() } else { String::new() }));
                            list.push(Value::Map(item));
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List existing Docker containers on host".to_string(),
            example: "system.list_docker_containers { as: $containers }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_import_docker(engine: &mut Engine) {
    engine.register(
        "box.import_docker",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut docker_id = String::new();
            let mut zeno_name = String::new();
            let mut target = "import_result".to_string();

            let mut as_compose = false;
            let mut preserve_volumes = true;
            for child in &node.children {
                let resolved = resolve_node_value(_engine, child, scope);
                match child.name.as_str() {
                    "docker_id" | "id" => docker_id = resolved.to_string_coerce(),
                    "zeno_name" | "name" => zeno_name = resolved.to_string_coerce(),
                    "as_compose" | "compose" => as_compose = resolved.to_bool(),
                    "preserve_volumes" | "keep_volumes" => preserve_volumes = resolved.to_bool(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if docker_id.is_empty() {
                let mut res = HashMap::new();
                res.insert("success".to_string(), Value::Bool(false));
                res.insert("stderr".to_string(), Value::String("docker_id is required".to_string()));
                scope.set(&target, Value::Map(res));
                return Ok(());
            }

            if zeno_name.is_empty() {
                zeno_name = docker_id.clone();
            }

            let inspect_out = Command::new("docker")
                .args(&["inspect", &docker_id])
                .output();

            let mut result = HashMap::new();
            if inspect_out.is_err() || !inspect_out.as_ref().unwrap().status.success() {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String(format!("Failed to inspect docker container '{}'", docker_id)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let inspect_val = inspect_out.unwrap();
            let inspect_str = String::from_utf8_lossy(&inspect_val.stdout);
            let inspect_json: serde_json::Value = match serde_json::from_str(&inspect_str) {
                Ok(v) => v,
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(format!("Failed to parse docker inspect JSON: {}", e)));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            };

            let cont_meta = match inspect_json.get(0) {
                Some(m) => m,
                None => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String("Docker inspect output empty".to_string()));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            };

            let image = cont_meta.get("Config")
                .and_then(|c| c.get("Image"))
                .and_then(|i| i.as_str())
                .unwrap_or("docker-imported:latest")
                .to_string();

            let mut cmd = Vec::new();
            if let Some(cmd_arr) = cont_meta.get("Config").and_then(|c| c.get("Cmd")).and_then(|c| c.as_array()) {
                for item in cmd_arr {
                    if let Some(s) = item.as_str() {
                        cmd.push(s.to_string());
                    }
                }
            }

            let mut env = HashMap::new();
            if let Some(env_arr) = cont_meta.get("Config").and_then(|c| c.get("Env")).and_then(|e| e.as_array()) {
                for item in env_arr {
                    if let Some(s) = item.as_str() {
                        let parts: Vec<&str> = s.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            env.insert(parts[0].to_string(), parts[1].to_string());
                        }
                    }
                }
            }

            let cwd = cont_meta.get("Config")
                .and_then(|c| c.get("WorkingDir"))
                .and_then(|w| w.as_str())
                .unwrap_or("/")
                .to_string();

            let mut ports = Vec::new();
            if let Some(port_bindings) = cont_meta.get("HostConfig").and_then(|h| h.get("PortBindings")).and_then(|p| p.as_object()) {
                for (cont_port, bindings) in port_bindings {
                    let c_port = cont_port.split('/').next().unwrap_or(cont_port);
                    if let Some(b_arr) = bindings.as_array() {
                        if let Some(b_first) = b_arr.get(0) {
                            if let Some(host_port) = b_first.get("HostPort").and_then(|hp| hp.as_str()) {
                                ports.push(format!("{}:{}", host_port, c_port));
                            }
                        }
                    }
                }
            }

            let mut mounts = Vec::new();
            if let Some(mount_arr) = cont_meta.get("Mounts").and_then(|m| m.as_array()) {
                for m in mount_arr {
                    let src = m.get("Source").and_then(|s| s.as_str()).unwrap_or("");
                    let dst = m.get("Destination").and_then(|d| d.as_str()).unwrap_or("");
                    if !src.is_empty() && !dst.is_empty() {
                        mounts.push(format!("{}:{}", src, dst));
                    }
                }
            }

            let data_dir = get_data_dir();

            if as_compose {
                let compose_dir = Path::new(&data_dir).join("compose").join(&zeno_name);
                if let Err(e) = fs::create_dir_all(&compose_dir) {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(format!("Failed to create compose directory: {}", e)));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }

                let mut yaml_content = format!("version: '3.8'\n\nservices:\n  {}:\n    image: {}\n    container_name: {}\n", zeno_name, image, zeno_name);
                if !cmd.is_empty() {
                    yaml_content.push_str(&format!("    command: {}\n", cmd.join(" ")));
                }
                if !ports.is_empty() {
                    yaml_content.push_str("    ports:\n");
                    for p in &ports {
                        yaml_content.push_str(&format!("      - '{}'\n", p));
                    }
                }
                if !env.is_empty() {
                    yaml_content.push_str("    environment:\n");
                    for (k, v) in &env {
                        yaml_content.push_str(&format!("      - {}={}\n", k, v));
                    }
                }
                if preserve_volumes && !mounts.is_empty() {
                    yaml_content.push_str("    volumes:\n");
                    for m in &mounts {
                        yaml_content.push_str(&format!("      - {}\n", m));
                    }
                }
                yaml_content.push_str("    restart: unless-stopped\n");

                let yaml_path = compose_dir.join("docker-compose.yml");
                if let Err(e) = fs::write(&yaml_path, yaml_content) {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(format!("Failed to write docker-compose.yml: {}", e)));
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }

                result.insert("success".to_string(), Value::Bool(true));
                result.insert("message".to_string(), Value::String(format!("Docker container imported as Zeno Box Compose project '{}'!", zeno_name)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let bundle_p = bundle_dir(&data_dir, &zeno_name);
            let rootfs_p = rootfs_dir(&data_dir, &zeno_name);

            if let Err(e) = fs::create_dir_all(&rootfs_p) {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String(format!("Failed to create rootfs directory: {}", e)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let export_status = Command::new("sh")
                .arg("-c")
                .arg(format!("docker export {} | tar -x -C {}", docker_id, rootfs_p.to_string_lossy()))
                .status();

            if export_status.is_err() || !export_status.as_ref().unwrap().success() {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String(format!("Failed to export docker container filesystem for '{}'", docker_id)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            if let Err(e) = generate_config_json(
                &bundle_p,
                cmd.clone(),
                env.clone(),
                &cwd,
                mounts.clone(),
                false,
                0,
                0.0,
                None,
                false,
            ) {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String(format!("Failed to generate OCI config.json: {}", e)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let c_log_path = log_path(&data_dir, &zeno_name).to_string_lossy().to_string();
            let state = ContainerState {
                id: zeno_name.clone(),
                image,
                status: "stopped".to_string(),
                pid: 0,
                created_at: chrono::Utc::now().to_rfc3339(),
                exited_at: None,
                exit_code: None,
                cmd,
                log_path: Some(c_log_path),
                ports: Some(ports),
                env: Some(env),
                mounts: Some(mounts),
                cwd: Some(cwd),
                host_network: Some(false),
                restart_policy: Some("unless-stopped".to_string()),
                desired_status: Some("stopped".to_string()),
                memory_limit: Some(0),
                cpu_limit: Some(0.0),
                oom_score_adj: None,
                read_only: Some(false),
                network: Some("bridge".to_string()),
            };

            if let Err(e) = save_container_state(&state) {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String(format!("Failed to save container state: {}", e)));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            result.insert("success".to_string(), Value::Bool(true));
            result.insert("message".to_string(), Value::String(format!("Container '{}' imported successfully into Zeno Box!", zeno_name)));
            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Import an existing Docker container into Zeno Box".to_string(),
            example: "box.import_docker: { docker_id: 'my-docker-db', zeno_name: 'zeno-db', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

