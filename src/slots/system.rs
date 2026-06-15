use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use sysinfo::{System, Disks, Networks, Pid};
use std::sync::Arc;
use std::collections::HashMap;

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

            let mut info = HashMap::new();
            info.insert("hostname".to_string(), Value::String(hostname));
            info.insert("cores".to_string(), Value::Int(cores));
            info.insert("uptime".to_string(), Value::String(uptime));
            info.insert("os".to_string(), Value::String(os_ver));
            info.insert("cpu_model".to_string(), Value::String(cpu_model));
            info.insert("platform".to_string(), Value::String(platform));
            info.insert("arch".to_string(), Value::String(arch));

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
            let mut rate_limit_max = 100;
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
}
