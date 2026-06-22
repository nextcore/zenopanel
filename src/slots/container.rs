use std::collections::HashMap;
use std::sync::Arc;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

pub fn register(engine: &mut Engine) {
    register_container_create(engine);
    register_container_start(engine);
    register_container_stop(engine);
    register_container_delete(engine);
    register_container_list(engine);
    register_container_pull(engine);
    register_container_images(engine);
    register_container_rootfs_path(engine);
    register_container_rmi(engine);
    register_container_inspect(engine);
    register_container_logs(engine);
    register_container_compose(engine);
    register_container_compose_get_yaml(engine);
}

/// Get the zeno-container binary path, checking env var ZENO_CONTAINER_BIN first.
fn get_container_bin() -> String {
    std::env::var("ZENO_CONTAINER_BIN")
        .unwrap_or_else(|_| "/usr/local/bin/zeno-container".to_string())
}

/// Get the zeno-container data directory, checking env var ZENO_CONTAINER_DATA_DIR first.
fn get_data_dir() -> String {
    std::env::var("ZENO_CONTAINER_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/zeno-container".to_string())
}

/// Parse JSON output from zeno-container CLI into a ZenoLang Value.
fn parse_json_output(json_str: &str) -> Value {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(val) => serde_json_to_zeno(&val),
        Err(_) => Value::String(json_str.to_string()),
    }
}

/// Convert serde_json::Value to ZenoLang Value.
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

/// Execute zeno-container CLI and return (stdout, stderr, exit_code) using async Command.
/// Uses block_in_place to prevent blocking the Tokio worker threads.
fn exec_zeno_container(args: &[&str]) -> (String, String, i32) {
    let bin = get_container_bin();
    let data_dir = get_data_dir();

    let mut all_args = vec!["--data-dir", &data_dir];
    all_args.extend_from_slice(args);

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let output = match tokio::process::Command::new(&bin).args(&all_args).output().await {
                Ok(out) => out,
                Err(e) => {
                    return (
                        String::new(),
                        format!("Failed to execute {}: {}", bin, e),
                        -1,
                    );
                }
            };

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            (stdout, stderr, exit_code)
        })
    })
}

/// container.create: Create a container with structured parameters.
fn register_container_create(engine: &mut Engine) {
    engine.register(
        "container.create",
        Arc::new(|engine, _ctx, node, scope| {
            let mut name = String::new();
            let mut image = String::new();
            let mut cmd = String::new();
            let mut ports: Vec<String> = Vec::new();
            let mut volumes: Vec<String> = Vec::new();
            let mut env_map: HashMap<String, String> = HashMap::new();
            let mut host_net = false;
            let mut target = "create_result".to_string();

            for child in &node.children {
                let child_name = &child.name;
                if child_name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                } else {
                    let resolved = engine.resolve_shorthand_value(child, scope);
                    match child_name.as_str() {
                        "name" => name = resolved.to_string_coerce(),
                        "image" => image = resolved.to_string_coerce(),
                        "cmd" => cmd = resolved.to_string_coerce(),
                        "host_net" => host_net = resolved.to_bool(),
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

            if name.is_empty() || image.is_empty() {
                let mut result = HashMap::new();
                result.insert("success".to_string(), Value::Bool(false));
                result.insert(
                    "stderr".to_string(),
                    Value::String("name and image are required".to_string()),
                );
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let mut cli_args = vec!["create", &name, "--image", &image];
            if !cmd.is_empty() {
                cli_args.push("--cmd");
                cli_args.push(&cmd);
            }
            for p in &ports {
                cli_args.push("--port");
                cli_args.push(p);
            }
            for v in &volumes {
                cli_args.push("--volume");
                cli_args.push(v);
            }
            let env_strings: Vec<String> = env_map.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            for e in &env_strings {
                cli_args.push("--env");
                cli_args.push(e);
            }
            if host_net {
                cli_args.push("--host-net");
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&cli_args);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Create a container with structured parameters".to_string(),
            example: "container.create { name: 'web', image: 'nginx', ports: ['80:80'] }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// helper function to extract single id input (value of slot or child parameter named "id")
fn resolve_id_param(engine: &Engine, node: &zenocore::Node, scope: &Arc<zenocore::Scope>) -> (String, String) {
    let mut id = String::new();
    let mut target = String::new();
    if node.value.is_some() {
        id = resolve_node_value(engine, node, scope).to_string_coerce();
    }
    for child in &node.children {
        if child.name == "id" {
            id = engine.resolve_shorthand_value(child, scope).to_string_coerce();
        } else if child.name == "as" {
            if let Some(ref val) = child.value {
                target = val.trim_start_matches('$').to_string();
            }
        }
    }
    (id, target)
}

/// container.start: Start a container.
fn register_container_start(engine: &mut Engine) {
    engine.register(
        "container.start",
        Arc::new(|engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(engine, node, scope);
            if target.is_empty() {
                target = "start_result".to_string();
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["start", &id]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Start a container".to_string(),
            example: "container.start: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.stop: Stop a container.
fn register_container_stop(engine: &mut Engine) {
    engine.register(
        "container.stop",
        Arc::new(|engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(engine, node, scope);
            if target.is_empty() {
                target = "stop_result".to_string();
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["stop", &id]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Stop a running container".to_string(),
            example: "container.stop: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.delete: Remove a container.
fn register_container_delete(engine: &mut Engine) {
    engine.register(
        "container.delete",
        Arc::new(|engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(engine, node, scope);
            if target.is_empty() {
                target = "delete_result".to_string();
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["rm", &id]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Delete a container".to_string(),
            example: "container.delete: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.list: List all containers.
fn register_container_list(engine: &mut Engine) {
    engine.register(
        "container.list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "containers".to_string();

            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let (stdout, _stderr, exit_code) = exec_zeno_container(&["ps", "--json"]);

            if exit_code != 0 {
                scope.set(&target, Value::List(Vec::new()));
                return Ok(());
            }

            let containers = parse_json_output(&stdout);
            scope.set(&target, containers);
            Ok(())
        }),
        SlotMeta {
            description: "List all containers".to_string(),
            example: "container.list { as: $containers }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.pull: Pull a container image.
fn register_container_pull(engine: &mut Engine) {
    engine.register(
        "container.pull",
        Arc::new(|engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "pull_result".to_string();

            if node.value.is_some() {
                let resolved = crate::slots::resolve_node_value(engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                let val = &child.name;
                if val == "image" {
                    if let Some(ref v) = child.value {
                        image = v.clone();
                    }
                } else if val == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            if image.is_empty() {
                let mut result = HashMap::new();
                result.insert("success".to_string(), Value::Bool(false));
                result.insert(
                    "error".to_string(),
                    Value::String("image is required".to_string()),
                );
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["pull", &image]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));
            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Pull a container image".to_string(),
            example: "container.pull: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.images: List cached images.
fn register_container_images(engine: &mut Engine) {
    engine.register(
        "container.images",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "images".to_string();

            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let (stdout, _stderr, exit_code) = exec_zeno_container(&["images"]);

            if exit_code != 0 {
                scope.set(&target, Value::List(Vec::new()));
                return Ok(());
            }

            let mut images = Vec::new();
            for line in stdout.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with("Cached") && !line.starts_with("No cached") {
                    let name = line.trim_start_matches("• ").trim().to_string();
                    if !name.is_empty() {
                        images.push(Value::String(name));
                    }
                }
            }

            scope.set(&target, Value::List(images));
            Ok(())
        }),
        SlotMeta {
            description: "List cached container images".to_string(),
            example: "container.images { as: $images }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.rootfs_path: Get the rootfs path for a container.
fn register_container_rootfs_path(engine: &mut Engine) {
    engine.register(
        "container.rootfs_path",
        Arc::new(|engine, _ctx, node, scope| {
            let mut container_id = String::new();
            let mut target = "rootfs_path".to_string();

            if node.value.is_some() {
                let resolved = crate::slots::resolve_node_value(engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    container_id = val_str;
                }
            }

            for child in &node.children {
                if child.name == "id" {
                    if let Some(ref v) = child.value {
                        container_id = v.clone();
                    }
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let path = format!(
                "{}/containers/{}/bundle/rootfs",
                get_data_dir(),
                container_id
            );
            scope.set(&target, Value::String(path));
            Ok(())
        }),
        SlotMeta {
            description: "Get container rootfs path".to_string(),
            example: "container.rootfs_path: 'my-container' { as: $path }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.rmi: Remove a cached image.
fn register_container_rmi(engine: &mut Engine) {
    engine.register(
        "container.rmi",
        Arc::new(|engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "rmi_result".to_string();

            if node.value.is_some() {
                let resolved = crate::slots::resolve_node_value(engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                let name = &child.name;
                if name == "image" {
                    if let Some(ref v) = child.value {
                        image = v.clone();
                    }
                } else if name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            if image.is_empty() {
                let mut result = HashMap::new();
                result.insert("success".to_string(), Value::Bool(false));
                result.insert(
                    "error".to_string(),
                    Value::String("image is required".to_string()),
                );
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["rmi", &image]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));
            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Remove a cached image".to_string(),
            example: "container.rmi: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.inspect: Inspect a container (parses config JSON).
fn register_container_inspect(engine: &mut Engine) {
    engine.register(
        "container.inspect",
        Arc::new(|engine, _ctx, node, scope| {
            let (id, mut target) = resolve_id_param(engine, node, scope);
            if target.is_empty() {
                target = "inspect_result".to_string();
            }

            let (stdout, stderr, exit_code) = exec_zeno_container(&["inspect", &id]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout.clone()));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));
            
            if exit_code == 0 {
                let parsed = parse_json_output(&stdout);
                result.insert("data".to_string(), parsed);
            } else {
                result.insert("data".to_string(), Value::Nil);
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Inspect container details".to_string(),
            example: "container.inspect: 'my-web' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.logs: Fetch container logs.
fn register_container_logs(engine: &mut Engine) {
    engine.register(
        "container.logs",
        Arc::new(|engine, _ctx, node, scope| {
            let mut id = String::new();
            let mut tail = 0;
            let mut target = "logs_result".to_string();

            if node.value.is_some() {
                id = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "id" => id = engine.resolve_shorthand_value(child, scope).to_string_coerce(),
                    "tail" => tail = engine.resolve_shorthand_value(child, scope).to_int(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let tail_str = tail.to_string();
            let args = if tail > 0 {
                vec!["logs", &id, "--tail", &tail_str]
            } else {
                vec!["logs", &id]
            };

            let (stdout, stderr, exit_code) = exec_zeno_container(&args);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Get container console logs".to_string(),
            example: "container.logs: 'my-web' { tail: 50, as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.compose: Handle compose up/down/ps safely with a unique temp file.
fn register_container_compose(engine: &mut Engine) {
    engine.register(
        "container.compose",
        Arc::new(|engine, _ctx, node, scope| {
            let mut action = String::new();
            let mut yaml = String::new();
            let mut target = "compose_result".to_string();

            if node.value.is_some() {
                action = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "action" => action = engine.resolve_shorthand_value(child, scope).to_string_coerce(),
                    "yaml" => yaml = engine.resolve_shorthand_value(child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if action.is_empty() {
                let mut result = HashMap::new();
                result.insert("success".to_string(), Value::Bool(false));
                result.insert(
                    "stderr".to_string(),
                    Value::String("action is required".to_string()),
                );
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let compose_dir = "/var/lib/zeno-container/compose";
            let compose_path = "/var/lib/zeno-container/compose/docker-compose.yml";

            if !yaml.is_empty() {
                if let Err(e) = std::fs::create_dir_all(compose_dir) {
                    let mut result = HashMap::new();
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert(
                        "stderr".to_string(),
                        Value::String(format!("Failed to create compose directory: {}", e)),
                    );
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }

                if let Err(e) = std::fs::write(compose_path, &yaml) {
                    let mut result = HashMap::new();
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert(
                        "stderr".to_string(),
                        Value::String(format!("Failed to write compose file: {}", e)),
                    );
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            } else {
                // If YAML is empty, verify that the physical compose file already exists
                if !std::path::Path::new(compose_path).exists() {
                    let mut result = HashMap::new();
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert(
                        "stderr".to_string(),
                        Value::String("Compose file does not exist on disk and no YAML was provided".to_string()),
                    );
                    scope.set(&target, Value::Map(result));
                    return Ok(());
                }
            }

            // Execute compose action on the physical file
            let (stdout, stderr, exit_code) = exec_zeno_container(&["compose", &action, compose_path]);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Run docker-compose commands safely using a physical compose file".to_string(),
            example: "container.compose: 'up' { yaml: $yaml, as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

fn register_container_compose_get_yaml(engine: &mut Engine) {
    engine.register(
        "container.compose_get_yaml",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();

            for child in &node.children {
                match child.name.as_str() {
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let compose_path = "/var/lib/zeno-container/compose/docker-compose.yml";
            let yaml_content = match std::fs::read_to_string(compose_path) {
                Ok(content) => content,
                Err(_) => String::new(),
            };

            let mut result = HashMap::new();
            result.insert("yaml".to_string(), Value::String(yaml_content));
            result.insert("success".to_string(), Value::Bool(true));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Get persistent docker-compose.yml content from disk".to_string(),
            example: "container.compose_get_yaml: { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}
