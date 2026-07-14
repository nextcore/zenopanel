use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use std::fs;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

use super::common::get_data_dir;

pub fn register(engine: &mut Engine) {
    register_volume_list(engine);
    register_volume_create(engine);
    register_volume_delete(engine);
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
