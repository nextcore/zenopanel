use std::collections::HashMap;
use std::sync::Arc;
use zenocore::{Engine, SlotMeta, Value};

pub fn register(engine: &mut Engine) {
    register_container_exec(engine);
    register_container_list(engine);
    register_container_pull(engine);
    register_container_images(engine);
    register_container_rootfs_path(engine);
    register_container_rmi(engine);
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

/// Execute zeno-container CLI and return (stdout, stderr, exit_code).
fn exec_zeno_container(args: &[&str]) -> (String, String, i32) {
    let bin = get_container_bin();
    let data_dir = get_data_dir();

    let mut all_args = vec!["--data-dir", &data_dir];
    all_args.extend_from_slice(args);

    let output = match std::process::Command::new(&bin).args(&all_args).output() {
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
}

/// container.exec: Execute an arbitrary zeno-container command.
fn register_container_exec(engine: &mut Engine) {
    engine.register(
        "container.exec",
        Arc::new(|engine, _ctx, node, scope| {
            let mut args: Vec<String> = Vec::new();
            let mut target = "container_result".to_string();

            if node.value.is_some() {
                let resolved = crate::slots::resolve_node_value(engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    args = val_str.split_whitespace().map(|s| s.to_string()).collect();
                }
            }

            for child in &node.children {
                let name = &child.name;
                if name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                } else {
                    let resolved = engine.resolve_shorthand_value(child, scope);
                    let val_str = resolved.to_string_coerce();
                    if !val_str.is_empty() && val_str != "<nil>" && !val_str.starts_with('$') {
                        args.push(name.clone());
                        args.push(val_str);
                    }
                }
            }

            let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let (stdout, stderr, exit_code) = exec_zeno_container(&args_refs);

            let mut result = HashMap::new();
            result.insert("stdout".to_string(), Value::String(stdout));
            result.insert("stderr".to_string(), Value::String(stderr));
            result.insert("exit_code".to_string(), Value::Int(exit_code as i64));
            result.insert("success".to_string(), Value::Bool(exit_code == 0));

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Execute zeno-container CLI command".to_string(),
            example: "container.exec: 'ps --json' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        },
    );
}

/// container.list: List all containers (calls `zeno-container ps --json`).
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

            // Parse JSON output — zeno-container ps --json returns an array
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

            // Parse lines into a list
            let mut images = Vec::new();
            for line in stdout.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with("Cached") && !line.starts_with("No cached")
                {
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
