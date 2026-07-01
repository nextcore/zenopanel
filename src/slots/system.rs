use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use sysinfo::{System, Disks, Networks, Pid};
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

static LATEST_VERSION: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static CHECKER_SPAWNED: std::sync::Once = std::sync::Once::new();

fn get_latest_version_cache() -> &'static Mutex<Option<String>> {
    LATEST_VERSION.get_or_init(|| Mutex::new(None))
}

fn spawn_update_checker() {
    CHECKER_SPAWNED.call_once(|| {
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .user_agent(concat!("ZenoPanel-Update-Checker/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            loop {
                // Query Raw Cargo.toml from GitHub
                match client.get("https://raw.githubusercontent.com/nextcore/zenopanel/main/Cargo.toml")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if let Ok(text) = resp.text().await {
                            if let Some(line) = text.lines().find(|l| l.trim().starts_with("version =")) {
                                if let Some(ver) = line.split('"').nth(1) {
                                    let ver_str = ver.to_string();
                                    *get_latest_version_cache().lock().unwrap() = Some(ver_str);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[Update Checker] Failed to check for updates: {}", e);
                    }
                }
                
                // Sleep for 12 hours
                tokio::time::sleep(tokio::time::Duration::from_secs(12 * 3600)).await;
            }
        });
    });
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    if days > 0 {
        format!("{} days, {} hours, {} mins", days, hours, minutes)
    } else if hours > 0 {
        format!("{} hours, {} mins", hours, minutes)
    } else {
        format!("{} mins", minutes)
    }
}

pub fn register(engine: &mut Engine) {
    engine.register(
        "system.info",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "sys_info".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let mut sys = System::new_all();
            sys.refresh_all();

            let hostname = System::host_name().unwrap_or_default();
            let cores = sys.cpus().len() as i64;
            let uptime_sec = System::uptime();
            let uptime = format_uptime(uptime_sec);
            let os_ver = System::long_os_version().unwrap_or_default();
            let cpu_model = sys.cpus().first().map(|cpu| cpu.brand().to_string()).unwrap_or_default();
            let platform = std::env::consts::OS.to_string();
            let arch = std::env::consts::ARCH.to_string();
            let username = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "zeno".to_string());

            spawn_update_checker();
            let latest_ver = get_latest_version_cache().lock().unwrap().clone();
            let current_ver = env!("CARGO_PKG_VERSION").to_string();

            let mut info = HashMap::new();
            info.insert("hostname".to_string(), Value::String(hostname));
            info.insert("cores".to_string(), Value::Int(cores));
            info.insert("uptime".to_string(), Value::String(uptime));
            info.insert("os".to_string(), Value::String(os_ver));
            info.insert("cpu_model".to_string(), Value::String(cpu_model));
            info.insert("platform".to_string(), Value::String(platform));
            info.insert("arch".to_string(), Value::String(arch));
            info.insert("username".to_string(), Value::String(username));
            info.insert("version".to_string(), Value::String(current_ver.clone()));

            if let Some(latest) = latest_ver {
                info.insert("latest_version".to_string(), Value::String(latest.clone()));
                let update_available = latest != current_ver;
                info.insert("update_available".to_string(), Value::Bool(update_available));
            } else {
                info.insert("update_available".to_string(), Value::Bool(false));
            }

            scope.set(&target, Value::Map(info));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.stats",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "sys_stats".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let mut sys = System::new_all();
            sys.refresh_cpu_usage();
            std::thread::sleep(std::time::Duration::from_millis(100));
            sys.refresh_cpu_usage();

            let cpu = sys.global_cpu_info().cpu_usage() as f64;
            let mem_total = sys.total_memory() as f64;
            let mem_free = sys.free_memory() as f64;
            let mem_used = sys.used_memory() as f64;
            let mem_pct = if mem_total > 0.0 { (mem_used / mem_total) * 100.0 } else { 0.0 };

            // Swap
            let swap_total = sys.total_swap() as f64;
            let swap_used = sys.used_swap() as f64;
            let swap_pct = if swap_total > 0.0 { (swap_used / swap_total) * 100.0 } else { 0.0 };

            let mut disk_total = 0.0;
            let mut disk_free = 0.0;
            let mut disk_used = 0.0;
            let mut disk_pct = 0.0;

            let disks = Disks::new_with_refreshed_list();
            if let Some(disk) = disks.iter().find(|d| d.mount_point() == std::path::Path::new("/")) {
                disk_total = disk.total_space() as f64;
                disk_free = disk.available_space() as f64;
                disk_used = disk_total - disk_free;
                if disk_total > 0.0 {
                    disk_pct = (disk_used / disk_total) * 100.0;
                }
            }

            let networks = Networks::new_with_refreshed_list();
            let mut net_rx = 0.0;
            let mut net_tx = 0.0;
            for (_interface_name, network) in &networks {
                net_rx += network.total_received() as f64;
                net_tx += network.total_transmitted() as f64;
            }

            let mut stats = HashMap::new();
            stats.insert("cpu".to_string(), Value::Float(cpu));
            stats.insert("mem_total".to_string(), Value::Float(mem_total));
            stats.insert("mem_free".to_string(), Value::Float(mem_free));
            stats.insert("mem_avail".to_string(), Value::Float(mem_free)); // fallback
            stats.insert("mem_used".to_string(), Value::Float(mem_used));
            stats.insert("mem_pct".to_string(), Value::Float(mem_pct));
            stats.insert("swap_total".to_string(), Value::Float(swap_total));
            stats.insert("swap_used".to_string(), Value::Float(swap_used));
            stats.insert("swap_pct".to_string(), Value::Float(swap_pct));
            stats.insert("disk_total".to_string(), Value::Float(disk_total));
            stats.insert("disk_free".to_string(), Value::Float(disk_free));
            stats.insert("disk_used".to_string(), Value::Float(disk_used));
            stats.insert("disk_pct".to_string(), Value::Float(disk_pct));
            stats.insert("net_rx".to_string(), Value::Float(net_rx));
            stats.insert("net_tx".to_string(), Value::Float(net_tx));

            scope.set(&target, Value::Map(stats));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.processes",
        Arc::new(|engine, _ctx, node, scope| {
            let mut target = "sys_processes".to_string();
            let mut sort_by = "mem".to_string();
            let mut limit = 50;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                } else if child.name == "sort" {
                    sort_by = val.to_string_coerce();
                } else if child.name == "limit" {
                    limit = val.to_int() as usize;
                }
            }

            let mut sys = System::new_all();
            sys.refresh_all();

            let mut procs: Vec<Value> = sys.processes().iter().map(|(pid, proc)| {
                let mut map = HashMap::new();
                map.insert("pid".to_string(), Value::Int(pid.as_u32() as i64));
                map.insert("name".to_string(), Value::String(proc.name().to_string()));
                map.insert("cpu".to_string(), Value::Float(proc.cpu_usage() as f64));
                
                let mem_bytes = proc.memory() as f64;
                let mem_pct = if sys.total_memory() > 0 {
                    (mem_bytes / sys.total_memory() as f64) * 100.0
                } else {
                    0.0
                };
                map.insert("memory".to_string(), Value::Float(mem_pct));
                map.insert("status".to_string(), Value::String(format!("{:?}", proc.status())));
                Value::Map(map)
            }).collect();

            if sort_by == "cpu" {
                procs.sort_by(|a, b| {
                    let a_val = a.to_map().get("cpu").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    let b_val = b.to_map().get("cpu").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    b_val.partial_cmp(&a_val).unwrap_or(std::cmp::Ordering::Equal)
                });
            } else {
                procs.sort_by(|a, b| {
                    let a_val = a.to_map().get("memory").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    let b_val = b.to_map().get("memory").cloned().unwrap_or(Value::Float(0.0)).to_float();
                    b_val.partial_cmp(&a_val).unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            if procs.len() > limit {
                procs.truncate(limit);
            }

            scope.set(&target, Value::List(procs));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.kill",
        Arc::new(|engine, _ctx, node, scope| {
            let mut pid = 0;
            let mut target = "kill_success".to_string();

            if node.value.is_some() {
                pid = resolve_node_value(engine, node, scope).to_int();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "pid" {
                    pid = val.to_int();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if pid <= 0 {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "system.kill: invalid pid".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.kill".to_string()),
                });
            }

            let sys = System::new_all();
            let sys_pid = Pid::from(pid as usize);
            let found = sys.process(sys_pid).is_some();
            let success = if let Some(proc) = sys.process(sys_pid) {
                proc.kill()
            } else {
                false
            };

            println!("SYSTEM.KILL: pid={}, found={}, success={}", pid, found, success);

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.port_check",
        Arc::new(|engine, _ctx, node, scope| {
            let mut port = 0;
            let mut target = "port_info".to_string();

            if node.value.is_some() {
                port = resolve_node_value(engine, node, scope).to_int();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "port" {
                    port = val.to_int();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if port <= 0 || port > 65535 {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "system.port_check: invalid port number".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.port_check".to_string()),
                });
            }

            let mut res = HashMap::new();
            res.insert("port".to_string(), Value::Int(port));

            let output = std::process::Command::new("lsof")
                .args(&["-i", &format!(":{}", port), "-s", "tcp:listen", "-F", "pc"])
                .output();

            let mut in_use = false;
            let mut pid = None;
            let mut name = None;

            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        if line.starts_with('p') {
                            if let Ok(p) = line[1..].parse::<i64>() {
                                pid = Some(p);
                                in_use = true;
                            }
                        } else if line.starts_with('c') {
                            name = Some(line[1..].to_string());
                        }
                    }
                }
            }

            res.insert("in_use".to_string(), Value::Bool(in_use));
            res.insert("pid".to_string(), match pid {
                Some(p) => Value::Int(p),
                None => Value::Nil,
            });
            res.insert("process_name".to_string(), match name {
                Some(n) => Value::String(n),
                None => Value::Nil,
            });

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.dir_list",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = ".".to_string();
            let mut target = "dir_list".to_string();

            if node.value.is_some() {
                path = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let entries = std::fs::read_dir(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("system.dir_list failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.dir_list".to_string()),
                }
            })?;

            let mut list = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(meta) = entry.metadata() {
                        let mut map = HashMap::new();
                        map.insert("name".to_string(), Value::String(entry.file_name().to_string_lossy().into_owned()));
                        map.insert("is_dir".to_string(), Value::Bool(meta.is_dir()));
                        map.insert("size".to_string(), Value::Int(meta.len() as i64));
                        
                        let mod_time = meta.modified().ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| {
                                let datetime: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()).unwrap_or_default();
                                datetime.to_rfc3339()
                            })
                            .unwrap_or_default();
                        map.insert("mod_time".to_string(), Value::String(mod_time));
                        
                        #[cfg(unix)]
                        let mode = {
                            use std::os::unix::fs::PermissionsExt;
                            format!("{:o}", meta.permissions().mode())
                        };
                        #[cfg(not(unix))]
                        let mode = format!("{:o}", meta.permissions().readonly() as u32);
                        
                        map.insert("mode".to_string(), Value::String(mode));
                        list.push(Value::Map(map));
                    }
                }
            }

            list.sort_by(|a, b| {
                let a_map = a.to_map();
                let b_map = b.to_map();
                let a_is_dir = a_map.get("is_dir").cloned().unwrap_or(Value::Bool(false)).to_bool();
                let b_is_dir = b_map.get("is_dir").cloned().unwrap_or(Value::Bool(false)).to_bool();
                if a_is_dir != b_is_dir {
                    b_is_dir.cmp(&a_is_dir)
                } else {
                    let a_name = a_map.get("name").cloned().unwrap_or(Value::Nil).to_string_coerce();
                    let b_name = b_map.get("name").cloned().unwrap_or(Value::Nil).to_string_coerce();
                    a_name.cmp(&b_name)
                }
            });

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.env",
        Arc::new(|engine, _ctx, node, scope| {
            let env_name = resolve_node_value(engine, node, scope).to_string_coerce();
            let mut target = "env_val".to_string();

            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let val = std::env::var(&env_name).unwrap_or_default();
            scope.set(&target, Value::String(val));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_status",
        Arc::new(|engine, _ctx, node, scope| {
            let mut service = String::new();
            let mut target = "service_status".to_string();

            if node.value.is_some() {
                service = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "service" {
                    service = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let output = std::process::Command::new("systemctl")
                .args(&["is-active", &service])
                .output();

            let (status, active) = match output {
                Ok(out) => {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let act = s == "active";
                    (s, act)
                }
                Err(_) => ("unknown".to_string(), false)
            };

            let mut res = HashMap::new();
            res.insert("service".to_string(), Value::String(service));
            res.insert("status".to_string(), Value::String(status));
            res.insert("active".to_string(), Value::Bool(active));

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.service_control",
        Arc::new(|engine, _ctx, node, scope| {
            let mut service = String::new();
            let mut action = String::new();
            let mut target = "control_result".to_string();

            if node.value.is_some() {
                service = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "service" {
                    service = val.to_string_coerce();
                } else if child.name == "action" {
                    action = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let valid_actions = ["start", "stop", "restart", "reload", "enable", "disable"];
            if !valid_actions.contains(&action.as_str()) {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("system.service_control: invalid action '{}'", action),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("system.service_control".to_string()),
                });
            }

            let output = std::process::Command::new("systemctl")
                .args(&[&action, &service])
                .output();

            let mut res = HashMap::new();
            match output {
                Ok(out) => {
                    let success = out.status.success();
                    res.insert("success".to_string(), Value::Bool(success));
                    if !success {
                        let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        res.insert("error".to_string(), Value::String(err_msg));
                    } else {
                        res.insert("error".to_string(), Value::String(String::new()));
                    }
                }
                Err(e) => {
                    res.insert("success".to_string(), Value::Bool(false));
                    res.insert("error".to_string(), Value::String(e.to_string()));
                }
            }

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.exec",
        Arc::new(|engine, _ctx, node, scope| {
            let mut command = String::new();
            let mut target = "exec_result".to_string();

            if node.value.is_some() {
                command = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "cmd" || child.name == "command" {
                    command = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let output = std::process::Command::new("bash")
                .args(&["-c", &command])
                .output();

            let mut res = HashMap::new();
            match output {
                Ok(out) => {
                    let exit_code = out.status.code().unwrap_or(-1);
                    res.insert("stdout".to_string(), Value::String(String::from_utf8_lossy(&out.stdout).to_string()));
                    res.insert("stderr".to_string(), Value::String(String::from_utf8_lossy(&out.stderr).to_string()));
                    res.insert("exit_code".to_string(), Value::Int(exit_code as i64));
                    res.insert("success".to_string(), Value::Bool(exit_code == 0));
                }
                Err(e) => {
                    res.insert("stdout".to_string(), Value::String(String::new()));
                    res.insert("stderr".to_string(), Value::String(e.to_string()));
                    res.insert("exit_code".to_string(), Value::Int(-1));
                    res.insert("success".to_string(), Value::Bool(false));
                }
            }

            scope.set(&target, Value::Map(res));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.get_security_settings",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "sec_settings".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.get_security_settings".to_string()) })?;

            let mut map = HashMap::new();
            map.insert("waf_enabled".to_string(), Value::Bool(app_state.waf_enabled.load(std::sync::atomic::Ordering::Relaxed)));
            map.insert("rate_limit_enabled".to_string(), Value::Bool(app_state.rate_limiter.is_enabled()));
            map.insert("rate_limit_max".to_string(), Value::Int(app_state.rate_limiter.max_requests() as i64));
            map.insert("rate_limit_window".to_string(), Value::Int(app_state.rate_limiter.window_secs() as i64));

            scope.set(&target, Value::Map(map));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.update_security_settings",
        Arc::new(|engine, ctx, node, scope| {
            let mut target = "success".to_string();
            let mut waf_enabled = true;
            let mut rate_limit_enabled = true;
            let mut rate_limit_max = 1000;
            let mut rate_limit_window = 60;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "waf_enabled" {
                    waf_enabled = val.to_bool();
                } else if child.name == "rate_limit_enabled" {
                    rate_limit_enabled = val.to_bool();
                } else if child.name == "rate_limit_max" {
                    rate_limit_max = val.to_int() as usize;
                } else if child.name == "rate_limit_window" {
                    rate_limit_window = val.to_int() as u64;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.update_security_settings".to_string()) })?;

            app_state.waf_enabled.store(waf_enabled, std::sync::atomic::Ordering::Relaxed);
            app_state.rate_limiter.update(rate_limit_enabled, rate_limit_max, rate_limit_window);

            let db_manager = app_state.db_manager.clone();
            tokio::spawn(async move {
                if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                    let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('waf_enabled', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                        .bind(if waf_enabled { "true" } else { "false" })
                        .execute(&pool)
                        .await;
                    let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_enabled', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                        .bind(if rate_limit_enabled { "true" } else { "false" })
                        .execute(&pool)
                        .await;
                    let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_max', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                        .bind(rate_limit_max.to_string())
                        .execute(&pool)
                        .await;
                    let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('rate_limit_window', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                        .bind(rate_limit_window.to_string())
                        .execute(&pool)
                        .await;
                }
            });

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.get_traffic_history",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "traffic_history".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.get_traffic_history".to_string()) })?;

            let history = app_state.traffic_stats.get_history();
            let mut zeno_list = Vec::new();
            for item in history {
                let mut map = HashMap::new();
                map.insert("timestamp".to_string(), Value::Int(item.timestamp as i64));
                map.insert("requests".to_string(), Value::Int(item.requests as i64));
                map.insert("bytes_sent".to_string(), Value::Int(item.bytes_sent as i64));
                map.insert("bytes_received".to_string(), Value::Int(item.bytes_received as i64));
                map.insert("latency_ms".to_string(), Value::Int(item.latency_ms as i64));
                zeno_list.push(Value::Map(map));
            }

            scope.set(&target, Value::List(zeno_list));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.get_backup_settings",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "backup_settings".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.get_backup_settings".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            let target_clone = target.clone();
            let scope_clone = scope.clone();
            
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        macro_rules! get_s {
                            ($k:expr) => {{
                                let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
                                    .bind($k)
                                    .fetch_optional(&pool)
                                    .await
                                    .unwrap_or(None);
                                v.map(|r| r.0).unwrap_or_default()
                            }};
                        }

                        let enabled = get_s!("backup_enabled") == "true";
                        let interval = get_s!("backup_interval_hours").parse::<i64>().unwrap_or(24);
                        let retention = get_s!("backup_retention").parse::<i64>().unwrap_or(7);
                        let dest_dir = get_s!("backup_dest_dir");
                        let post_script = get_s!("backup_post_script");
                        let last_run = get_s!("backup_last_run");
                        let last_status = get_s!("backup_last_status");

                        let mut map = HashMap::new();
                        map.insert("enabled".to_string(), Value::Bool(enabled));
                        map.insert("interval_hours".to_string(), Value::Int(interval));
                        map.insert("retention".to_string(), Value::Int(retention));
                        map.insert("dest_dir".to_string(), Value::String(dest_dir));
                        map.insert("post_script".to_string(), Value::String(post_script));
                        map.insert("last_run".to_string(), Value::String(last_run));
                        map.insert("last_status".to_string(), Value::String(last_status));

                        scope_clone.set(&target_clone, Value::Map(map));
                    }
                });
            });

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.update_backup_settings",
        Arc::new(|engine, ctx, node, scope| {
            let mut target = "success".to_string();
            let mut enabled = false;
            let mut interval = 24;
            let mut retention = 7;
            let mut dest_dir = String::new();
            let mut post_script = String::new();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "enabled" {
                    enabled = val.to_bool();
                } else if child.name == "interval_hours" {
                    interval = val.to_int();
                } else if child.name == "retention" {
                    retention = val.to_int();
                } else if child.name == "dest_dir" {
                    dest_dir = val.to_string_coerce();
                } else if child.name == "post_script" {
                    post_script = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.update_backup_settings".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_enabled', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(if enabled { "true" } else { "false" })
                            .execute(&pool)
                            .await;
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_interval_hours', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(interval.to_string())
                            .execute(&pool)
                            .await;
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_retention', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(retention.to_string())
                            .execute(&pool)
                            .await;
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_dest_dir', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(&dest_dir)
                            .execute(&pool)
                            .await;
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_post_script', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(&post_script)
                            .execute(&pool)
                            .await;
                    }
                });
            });

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.trigger_backup",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.trigger_backup".to_string()) })?;

            let backup_mgr = app_state.backup_manager.clone();
            
            let mut result = HashMap::new();
            
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    backup_mgr.run_backup().await
                })
            });

            match res {
                Ok(filename) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("filename".to_string(), Value::String(filename.clone()));
                    
                    let db_manager = app_state.db_manager.clone();
                    let run_time = chrono::Utc::now().to_rfc3339();
                    let success_status = format!("Success (Filename: {})", filename);
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                                let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_last_run', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                    .bind(&run_time)
                                    .execute(&pool)
                                    .await;
                                let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_last_status', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                    .bind(&success_status)
                                    .execute(&pool)
                                    .await;
                            }
                        });
                    });
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("error".to_string(), Value::String(e.to_string()));
                    
                    let db_manager = app_state.db_manager.clone();
                    let run_time = chrono::Utc::now().to_rfc3339();
                    let failed_status = format!("Failed: {}", e);
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                                let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_last_run', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                    .bind(&run_time)
                                    .execute(&pool)
                                    .await;
                                let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('backup_last_status', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                    .bind(&failed_status)
                                    .execute(&pool)
                                    .await;
                            }
                        });
                    });
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.read_container_log ────────────────────────────────────────────
    engine.register(
        "system.read_container_log",
        Arc::new(|engine, _ctx, node, scope| {
            let mut target = "container_log".to_string();
            let mut container_id = String::new();
            let mut lines: usize = 100;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "as" => target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string(),
                    "id" => container_id = val.to_string_coerce(),
                    "lines" => lines = val.to_int() as usize,
                    _ => {}
                }
            }
            // Also accept bare positional value
            if container_id.is_empty() {
                if let Some(v) = &node.value {
                    container_id = v.trim_start_matches('$').to_string();
                    if let Some(resolved) = scope.get(&container_id) {
                        container_id = resolved.to_string_coerce();
                    } else {
                        container_id = v.clone();
                    }
                }
            }

            let base = std::env::var("ZENO_CONTAINER_DATA_DIR")
                .unwrap_or_else(|_| "/var/lib/zeno-container".to_string());
            let log_path = std::path::PathBuf::from(&base)
                .join("containers")
                .join(&container_id)
                .join("console.log");

            let mut result = HashMap::new();
            if log_path.exists() {
                match std::fs::read_to_string(&log_path) {
                    Ok(content) => {
                        let all_lines: Vec<&str> = content.lines().collect();
                        let start = if all_lines.len() > lines { all_lines.len() - lines } else { 0 };
                        let tail: Vec<Value> = all_lines[start..]
                            .iter()
                            .map(|l| Value::String(l.to_string()))
                            .collect();
                        result.insert("ok".to_string(), Value::Bool(true));
                        result.insert("lines".to_string(), Value::List(tail));
                        result.insert("path".to_string(), Value::String(log_path.to_string_lossy().to_string()));
                    }
                    Err(e) => {
                        result.insert("ok".to_string(), Value::Bool(false));
                        result.insert("error".to_string(), Value::String(e.to_string()));
                        result.insert("lines".to_string(), Value::List(vec![]));
                    }
                }
            } else {
                result.insert("ok".to_string(), Value::Bool(false));
                result.insert("error".to_string(), Value::String("Log file not found".to_string()));
                result.insert("lines".to_string(), Value::List(vec![]));
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.get_log_settings ──────────────────────────────────────────────
    engine.register(
        "system.get_log_settings",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "log_settings".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.get_log_settings".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            let target_clone = target.clone();
            let scope_clone = scope.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        macro_rules! get_s {
                            ($k:expr) => {{
                                let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
                                    .bind($k)
                                    .fetch_optional(&pool)
                                    .await
                                    .unwrap_or(None);
                                v.map(|r| r.0).unwrap_or_default()
                            }};
                        }

                        let interval_hours = get_s!("log_rotation_interval_hours").parse::<i64>().unwrap_or(24);
                        let max_size_mb = get_s!("log_max_size_mb").parse::<i64>().unwrap_or(10);
                        let waf_retention_days = get_s!("waf_log_retention_days").parse::<i64>().unwrap_or(30);
                        let last_rotation = get_s!("log_last_rotation");
                        let last_status = get_s!("log_last_status");

                        let mut map = HashMap::new();
                        map.insert("interval_hours".to_string(), Value::Int(interval_hours));
                        map.insert("max_size_mb".to_string(), Value::Int(max_size_mb));
                        map.insert("waf_retention_days".to_string(), Value::Int(waf_retention_days));
                        map.insert("last_rotation".to_string(), Value::String(last_rotation));
                        map.insert("last_status".to_string(), Value::String(last_status));

                        scope_clone.set(&target_clone, Value::Map(map));
                    }
                });
            });

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.update_log_settings ───────────────────────────────────────────
    engine.register(
        "system.update_log_settings",
        Arc::new(|engine, ctx, node, scope| {
            let mut target = "log_update_result".to_string();
            let mut interval_hours: i64 = 24;
            let mut max_size_mb: i64 = 10;
            let mut waf_retention_days: i64 = 30;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "as" => target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string(),
                    "interval_hours" => interval_hours = val.to_int(),
                    "max_size_mb" => max_size_mb = val.to_int(),
                    "waf_retention_days" => waf_retention_days = val.to_int(),
                    _ => {}
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.update_log_settings".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            let target_clone = target.clone();
            let scope_clone = scope.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        macro_rules! upsert {
                            ($k:expr, $v:expr) => {
                                let _ = sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                    .bind($k)
                                    .bind($v)
                                    .execute(&pool)
                                    .await;
                            };
                        }
                        upsert!("log_rotation_interval_hours", interval_hours.to_string());
                        upsert!("log_max_size_mb", max_size_mb.to_string());
                        upsert!("waf_log_retention_days", waf_retention_days.to_string());

                        let mut res = HashMap::new();
                        res.insert("ok".to_string(), Value::Bool(true));
                        scope_clone.set(&target_clone, Value::Map(res));
                    }
                });
            });

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.trigger_log_rotation ──────────────────────────────────────────
    engine.register(
        "system.trigger_log_rotation",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "log_rotation_result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.trigger_log_rotation".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            let log_manager = app_state.log_manager.clone();
            let target_clone = target.clone();
            let scope_clone = scope.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let max_size_mb = if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'log_max_size_mb'")
                            .fetch_optional(&pool).await.unwrap_or(None);
                        v.map(|r| r.0).unwrap_or("10".to_string()).parse::<u64>().unwrap_or(10)
                    } else { 10 };

                    let waf_retention_days = if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'waf_log_retention_days'")
                            .fetch_optional(&pool).await.unwrap_or(None);
                        v.map(|r| r.0).unwrap_or("30".to_string()).parse::<i64>().unwrap_or(30)
                    } else { 30 };

                    let summary = log_manager.run_rotation(max_size_mb, waf_retention_days).await;
                    let ts = chrono::Utc::now().to_rfc3339();

                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('log_last_rotation', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(&ts).execute(&pool).await;
                        let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('log_last_status', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                            .bind(&summary).execute(&pool).await;
                    }

                    let mut res = HashMap::new();
                    res.insert("ok".to_string(), Value::Bool(true));
                    res.insert("summary".to_string(), Value::String(summary));
                    res.insert("timestamp".to_string(), Value::String(ts));
                    scope_clone.set(&target_clone, Value::Map(res));
                });
            });

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.firewall_status ────────────────────────────────────────────────
    engine.register(
        "system.firewall_status",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "firewall_rules".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let output = std::process::Command::new("iptables")
                .args(&["-S", "INPUT"])
                .output();

            let mut list = Vec::new();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        if line.contains("ZenoPanel:") {
                            let comment_marker = "ZenoPanel:";
                            let name = if let Some(idx) = line.find(comment_marker) {
                                let mut name_part = line[idx + comment_marker.len()..].trim();
                                if name_part.ends_with('"') || name_part.ends_with('\'') {
                                    name_part = &name_part[..name_part.len() - 1];
                                }
                                name_part.trim().to_string()
                            } else {
                                "Unknown".to_string()
                            };

                            let protocol = if line.contains("-p tcp") {
                                "tcp".to_string()
                            } else if line.contains("-p udp") {
                                "udp".to_string()
                            } else {
                                "all".to_string()
                            };

                            let port = if let Some(idx) = line.find("--dport ") {
                                let port_part = line[idx + 8..].split_whitespace().next().unwrap_or("");
                                port_part.to_string()
                            } else {
                                "all".to_string()
                            };

                            let action = if line.contains("-j ACCEPT") {
                                "ACCEPT".to_string()
                            } else if line.contains("-j DROP") {
                                "DROP".to_string()
                            } else if line.contains("-j REJECT") {
                                "REJECT".to_string()
                            } else {
                                "UNKNOWN".to_string()
                            };

                            let mut rule_map = HashMap::new();
                            rule_map.insert("name".to_string(), Value::String(name));
                            rule_map.insert("port".to_string(), Value::String(port));
                            rule_map.insert("protocol".to_string(), Value::String(protocol));
                            rule_map.insert("action".to_string(), Value::String(action));
                            list.push(Value::Map(rule_map));
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.firewall_rule_add ──────────────────────────────────────────────
    engine.register(
        "system.firewall_rule_add",
        Arc::new(|engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut port = String::new();
            let mut protocol = "tcp".to_string();
            let mut action = "ACCEPT".to_string();
            let mut target = "success".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "name" => name = val.to_string_coerce(),
                    "port" => port = val.to_string_coerce(),
                    "protocol" => protocol = val.to_string_coerce(),
                    "action" => action = val.to_string_coerce(),
                    "as" => target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string(),
                    _ => {}
                }
            }

            let action_upper = action.to_uppercase();
            if action_upper == "DROP" || action_upper == "REJECT" {
                if port == "22" || port == "3000" || port == "8443" || port == "3001" || port == "3002" {
                    return Err(Diagnostic {
                        r#type: "error".to_string(),
                        message: "Lockout Protection: Memblokir port SSH (22) atau port manajemen ZenoPanel tidak diijinkan.".to_string(),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("system.firewall_rule_add".to_string()),
                    });
                }
            }

            // Check if rule already exists to prevent duplicate entries
            let check = std::process::Command::new("iptables")
                .args(&[
                    "-C", "INPUT",
                    "-p", &protocol,
                    "--dport", &port,
                    "-j", &action_upper,
                    "-m", "comment",
                    "--comment", &format!("ZenoPanel: {}", name)
                ])
                .output();
            let exists = check.is_ok() && check.unwrap().status.success();
            
            let success = if !exists {
                let output = std::process::Command::new("iptables")
                    .args(&[
                        "-A", "INPUT",
                        "-p", &protocol,
                        "--dport", &port,
                        "-j", &action_upper,
                        "-m", "comment",
                        "--comment", &format!("ZenoPanel: {}", name)
                    ])
                    .output();
                output.is_ok() && output.unwrap().status.success()
            } else {
                true
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    // ── system.firewall_rule_delete ───────────────────────────────────────────
    engine.register(
        "system.firewall_rule_delete",
        Arc::new(|engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut port = String::new();
            let mut protocol = "tcp".to_string();
            let mut action = "ACCEPT".to_string();
            let mut target = "success".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "name" => name = val.to_string_coerce(),
                    "port" => port = val.to_string_coerce(),
                    "protocol" => protocol = val.to_string_coerce(),
                    "action" => action = val.to_string_coerce(),
                    "as" => target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string(),
                    _ => {}
                }
            }

            let action_upper = action.to_uppercase();
            let output = std::process::Command::new("iptables")
                .args(&[
                    "-D", "INPUT",
                    "-p", &protocol,
                    "--dport", &port,
                    "-j", &action_upper,
                    "-m", "comment",
                    "--comment", &format!("ZenoPanel: {}", name)
                ])
                .output();
            let success = output.is_ok() && output.unwrap().status.success();

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.update_panel",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "success".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            // Spawn updater detached in background
            let child = std::process::Command::new("nohup")
                .args(&["sh", "-c", "sleep 1 && curl -fsSL https://raw.githubusercontent.com/nextcore/zenopanel/main/install.sh | bash"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            let success = child.is_ok();
            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.fetch_docker_tags",
        Arc::new(|engine, _ctx, node, scope| {
            let mut repo = String::new();
            let mut target = "tags".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "repo" {
                    repo = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if repo.is_empty() {
                scope.set(&target, Value::List(Vec::new()));
                return Ok(());
            }

            let full_repo = if repo.contains('/') {
                repo
            } else {
                format!("library/{}", repo)
            };

            let handle = tokio::runtime::Handle::current();
            let mut tag_list = Vec::new();
            
            let url = format!(
                "https://registry.hub.docker.com/v2/repositories/{}/tags?page_size=100",
                full_repo
            );

            let fetch_fut = async {
                let client = reqwest::Client::builder()
                    .user_agent("ZenoPanel-Tags-Fetcher/1.0")
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        return Some(json);
                    }
                }
                None
            };

            if let Some(json) = handle.block_on(fetch_fut) {
                if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
                    for res in results {
                        if let Some(name) = res.get("name").and_then(|n| n.as_str()) {
                            let tag_str = name.to_string();
                            if tag_str.len() > 20 && tag_str.chars().all(|c| c.is_ascii_hexdigit()) {
                                continue;
                            }
                            let lower = tag_str.to_lowercase();
                            if lower.contains("beta") || lower.contains("alpha") || lower.contains("rc") || 
                               lower.contains("dev") || lower == "latest" || lower == "master" || lower == "main" {
                                continue;
                            }
                            tag_list.push(Value::String(tag_str));
                        }
                    }
                }
            }

            scope.set(&target, Value::List(tag_list));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.firewall_get_lockdown",
        Arc::new(|_engine, ctx, node, scope| {
            let mut target = "lockdown_enabled".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.firewall_get_lockdown".to_string()) })?;

            let db_manager = app_state.db_manager.clone();
            let mut enabled = false;

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'firewall_lockdown'")
                            .fetch_optional(&pool)
                            .await
                            .unwrap_or(None);
                        if let Some(r) = v {
                            enabled = r.0 == "true";
                        }
                    }
                });
            });

            scope.set(&target, Value::Bool(enabled));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "system.firewall_set_lockdown",
        Arc::new(|engine, ctx, node, scope| {
            let mut target = "success".to_string();
            let mut enabled = false;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "enabled" {
                    enabled = val.to_bool();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let app_state = ctx.get::<Arc<crate::AppState>>("app_state")
                .map(|s| s.clone())
                .ok_or_else(|| Diagnostic { r#type: "error".to_string(), message: "AppState not found in Context".to_string(), filename: node.filename.clone(), line: node.line, col: node.col, slot: Some("system.firewall_set_lockdown".to_string()) })?;            let db_manager = app_state.db_manager.clone();
            let mut lockdown_ports_str = "22,80,443,3002".to_string();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                        let v: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'firewall_lockdown_ports'")
                            .fetch_optional(&pool)
                            .await
                            .unwrap_or(None);
                        if let Some(r) = v {
                            lockdown_ports_str = r.0;
                        }
                    }
                });
            });

            let ports: Vec<String> = lockdown_ports_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let success = if enabled {
                let check_and_add = |args: &[&str]| {
                    let mut check_args = vec!["-C"];
                    check_args.extend_from_slice(&args[1..]);
                    let check = std::process::Command::new("iptables").args(&check_args).output();
                    let exists = check.is_ok() && check.unwrap().status.success();
                    if !exists {
                        let _ = std::process::Command::new("iptables").args(args).output();
                    }
                };

                // 1. Allow loopback
                check_and_add(&["-A", "INPUT", "-i", "lo", "-j", "ACCEPT", "-m", "comment", "--comment", "ZenoPanel: Allow Loopback"]);

                // 2. Allow Established & Related
                check_and_add(&["-A", "INPUT", "-m", "conntrack", "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT", "-m", "comment", "--comment", "ZenoPanel: Allow Established/Related"]);

                // 3. Allow specified ports
                for port in &ports {
                    let rule_name = match port.as_str() {
                        "22" => "Allow SSH (Port 22)".to_string(),
                        "80" => "Allow HTTP (Port 80)".to_string(),
                        "443" => "Allow HTTPS (Port 443)".to_string(),
                        "3002" => "Allow ZenoPanel (Port 3002)".to_string(),
                        _ => format!("Allow Port {}", port),
                    };
                    check_and_add(&["-A", "INPUT", "-p", "tcp", "--dport", port, "-j", "ACCEPT", "-m", "comment", "--comment", &format!("ZenoPanel: {}", rule_name)]);
                }

                // 4. Set default policy INPUT to DROP
                let drop_policy = std::process::Command::new("iptables")
                    .args(&["-P", "INPUT", "DROP"])
                    .output();

                drop_policy.is_ok() && drop_policy.unwrap().status.success()
            } else {
                // 1. Reset default policy INPUT to ACCEPT
                let accept_policy = std::process::Command::new("iptables")
                    .args(&["-P", "INPUT", "ACCEPT"])
                    .output();

                // 2. Delete loopback rule
                let _ = std::process::Command::new("iptables")
                    .args(&["-D", "INPUT", "-i", "lo", "-j", "ACCEPT", "-m", "comment", "--comment", "ZenoPanel: Allow Loopback"])
                    .output();

                // 3. Delete Established & Related rule
                let _ = std::process::Command::new("iptables")
                    .args(&["-D", "INPUT", "-m", "conntrack", "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT", "-m", "comment", "--comment", "ZenoPanel: Allow Established/Related"])
                    .output();

                // 4. Delete specified ports
                for port in &ports {
                    let rule_name = match port.as_str() {
                        "22" => "Allow SSH (Port 22)".to_string(),
                        "80" => "Allow HTTP (Port 80)".to_string(),
                        "443" => "Allow HTTPS (Port 443)".to_string(),
                        "3002" => "Allow ZenoPanel (Port 3002)".to_string(),
                        _ => format!("Allow Port {}", port),
                    };
                    let _ = std::process::Command::new("iptables")
                        .args(&["-D", "INPUT", "-p", "tcp", "--dport", port, "-j", "ACCEPT", "-m", "comment", "--comment", &format!("ZenoPanel: {}", rule_name)])
                        .output();
                }

                accept_policy.is_ok() && accept_policy.unwrap().status.success()
            };
            if success {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        if let Some(crate::db::DbPool::Sqlite(pool)) = db_manager.get_pool("default").await {
                            let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('firewall_lockdown', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                                .bind(if enabled { "true" } else { "false" })
                                .execute(&pool)
                                .await;
                        }
                    });
                });
            }

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

