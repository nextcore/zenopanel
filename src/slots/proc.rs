use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use std::sync::Arc;
use std::collections::HashMap;

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
}
