use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use std::fs;

fn proc_info_to_value(info: &crate::procman::ProcessInfo) -> Value {
    let mut map = HashMap::new();
    map.insert("id".to_string(), Value::String(info.id.clone()));
    map.insert("name".to_string(), Value::String(info.name.clone()));
    map.insert("command".to_string(), Value::String(info.command.clone()));
    map.insert("cwd".to_string(), Value::String(info.cwd.clone()));
    
    let mut env_map = HashMap::new();
    for (k, v) in &info.env {
        env_map.insert(k.clone(), Value::String(v.clone()));
    }
    map.insert("env".to_string(), Value::Map(env_map));
    map.insert("auto_restart".to_string(), Value::Bool(info.auto_restart));
    map.insert("status".to_string(), Value::String(info.status.clone()));
    map.insert("pid".to_string(), match info.pid {
        Some(p) => Value::Int(p as i64),
        None => Value::Nil,
    });
    map.insert("exit_code".to_string(), match info.exit_code {
        Some(e) => Value::Int(e as i64),
        None => Value::Nil,
    });
    map.insert("cpu_usage".to_string(), Value::Float(info.cpu_usage as f64));
    map.insert("memory_usage".to_string(), Value::Float(info.memory_usage as f64));
    map.insert("port".to_string(), match info.port {
        Some(p) => Value::Int(p as i64),
        None => Value::Nil,
    });
    map.insert("ports".to_string(), Value::List(info.ports.iter().map(|p| Value::Int(*p as i64)).collect()));
    Value::Map(map)
}

pub fn register(engine: &mut Engine) {
    engine.register(
        "proc.list",
        Arc::new(|_engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.list: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.list".to_string()),
                }
            })?;

            let mut target = "processes".to_string();
            for child in &node.children {
                if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let list_fut = pm.list_processes();
            let list = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(list_fut)
            });

            let val_list = Value::List(list.iter().map(proc_info_to_value).collect());
            scope.set(&target, val_list);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.add",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.add: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.add".to_string()),
                }
            })?;

            let mut name = String::new();
            let mut command = String::new();
            let mut cwd = ".".to_string();
            let mut env = HashMap::new();
            let mut auto_restart = true;
            let mut port = None;
            let mut target = "id".to_string();

            if node.value.is_some() {
                name = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "name" {
                    name = val.to_string_coerce();
                } else if child.name == "command" || child.name == "cmd" {
                    command = val.to_string_coerce();
                } else if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "auto_restart" {
                    auto_restart = val.to_bool();
                } else if child.name == "port" {
                    let port_val = val.to_int();
                    if port_val > 0 && port_val <= 65535 {
                        port = Some(port_val as u16);
                    }
                } else if child.name == "env" {
                    if let Value::Map(m) = val {
                        for (k, v) in m {
                            env.insert(k, v.to_string_coerce());
                        }
                    } else {
                        for env_child in &child.children {
                            let env_val = engine.resolve_shorthand_value(env_child, scope);
                            env.insert(env_child.name.clone(), env_val.to_string_coerce());
                        }
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let add_fut = pm.add_process(name, command, cwd, env, auto_restart, port);
            let id = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(add_fut)
            }).map_err(|e| Diagnostic {
                r#type: "error".to_string(),
                message: format!("proc.add failed: {}", e),
                filename: node.filename.clone(),
                line: node.line,
                col: node.col,
                slot: Some("proc.add".to_string()),
            })?;

            scope.set(&target, Value::String(id));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.update",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.update: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.update".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut name = String::new();
            let mut command = String::new();
            let mut cwd = ".".to_string();
            let mut env = HashMap::new();
            let mut auto_restart = true;
            let mut port = None;
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "name" {
                    name = val.to_string_coerce();
                } else if child.name == "command" || child.name == "cmd" {
                    command = val.to_string_coerce();
                } else if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "auto_restart" {
                    auto_restart = val.to_bool();
                } else if child.name == "port" {
                    let port_val = val.to_int();
                    if port_val > 0 && port_val <= 65535 {
                        port = Some(port_val as u16);
                    }
                } else if child.name == "env" {
                    if let Value::Map(m) = val {
                        for (k, v) in m {
                            env.insert(k, v.to_string_coerce());
                        }
                    } else {
                        for env_child in &child.children {
                            let env_val = engine.resolve_shorthand_value(env_child, scope);
                            env.insert(env_child.name.clone(), env_val.to_string_coerce());
                        }
                    }
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let update_fut = pm.update_process(&id, name, command, cwd, env, auto_restart, port);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(update_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.start",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.start: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.start".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let start_fut = pm.start_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(start_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.stop",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.stop: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.stop".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let stop_fut = pm.stop_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(stop_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.restart",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.restart: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.restart".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let restart_fut = pm.restart_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(restart_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.delete",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.delete: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.delete".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut target = "success".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let delete_fut = pm.remove_process(&id);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(delete_fut)
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::Bool(false));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "proc.logs",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.logs: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.logs".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut lines = 100;
            let mut target = "logs".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "lines" {
                    lines = val.to_int() as usize;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let logs_fut = pm.get_logs(&id, lines);
            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(logs_fut)
            });

            match res {
                Ok(l) => {
                    scope.set(&target, Value::List(l.into_iter().map(Value::String).collect()));
                    scope.set("error", Value::Nil);
                }
                Err(e) => {
                    scope.set(&target, Value::List(Vec::new()));
                    scope.set("error", Value::String(e));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    register_proc_git_get(engine);
    register_proc_git_save(engine);
    register_proc_git_sync(engine);
}

fn register_proc_git_get(engine: &mut Engine) {
    engine.register(
        "proc.git_get",
        Arc::new(|engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut cwd = String::new();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if node.value.is_some() {
                cwd = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            let git_config_path = Path::new(&cwd).join(".zeno-git.json");
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
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

fn register_proc_git_save(engine: &mut Engine) {
    engine.register(
        "proc.git_save",
        Arc::new(|engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut cwd = String::new();
            let mut repo_url = String::new();
            let mut branch = "main".to_string();
            let mut webhook_token = String::new();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "cwd" {
                    cwd = val.to_string_coerce();
                } else if child.name == "repo_url" {
                    repo_url = val.to_string_coerce();
                } else if child.name == "branch" {
                    branch = val.to_string_coerce();
                } else if child.name == "webhook_token" {
                    webhook_token = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let git_config_path = Path::new(&cwd).join(".zeno-git.json");
            let mut success = false;
            if fs::create_dir_all(&cwd).is_ok() {
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
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

fn register_proc_git_sync(engine: &mut Engine) {
    engine.register(
        "proc.git_sync",
        Arc::new(|engine, ctx, node, scope| {
            let pm = ctx.get::<Arc<crate::procman::ProcessManager>>("process_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "proc.git_sync: ProcessManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("proc.git_sync".to_string()),
                }
            })?;

            let mut id = String::new();
            let mut token = String::new();
            let mut target = "result".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "id" {
                    id = val.to_string_coerce();
                } else if child.name == "token" {
                    token = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let mut result = HashMap::new();
            result.insert("success".to_string(), Value::Bool(false));

            let list_fut = pm.list_processes();
            let list = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(list_fut)
            });

            let proc_info = list.iter().find(|p| p.id == id);
            if proc_info.is_none() {
                result.insert("stderr".to_string(), Value::String("Process not found".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let cwd = &proc_info.unwrap().cwd;
            let git_config_path = Path::new(cwd).join(".zeno-git.json");

            if !git_config_path.exists() {
                result.insert("stderr".to_string(), Value::String("No Git configuration found for this process".to_string()));
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
            let saved_token = config.get("webhook_token").and_then(|v| v.as_str()).unwrap_or("");

            if !token.is_empty() && token != saved_token {
                result.insert("stderr".to_string(), Value::String("Unauthorized: Token mismatch".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            if repo_url.is_empty() {
                result.insert("stderr".to_string(), Value::String("Git repository URL is empty".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let volume_path = Path::new(cwd);
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
                    let restart_fut = pm.restart_process(&id);
                    let restart_res = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(restart_fut)
                    });

                    match restart_res {
                        Ok(_) => {
                            result.insert("success".to_string(), Value::Bool(true));
                            result.insert("stdout".to_string(), Value::String(format!("{}\n\nProcess restarted successfully", stdout_msg)));
                        }
                        Err(e) => {
                            result.insert("stderr".to_string(), Value::String(format!("Git pulled but process restart failed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}
