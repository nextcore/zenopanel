use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use serde::{Serialize, Deserialize};
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

use super::common::{
    get_data_dir, container_dir, rootfs_dir, parse_image_ref
};
use super::container::{
    container_create, container_start, container_stop, container_delete,
    container_list_internal
};
use super::image::{pull_image_rust, get_image_default_cmd};

pub fn register(engine: &mut Engine) {
    register_box_compose(engine);
    register_box_compose_get_yaml(engine);
    register_box_compose_list_projects(engine);
    register_box_compose_delete_project(engine);
    register_box_registry_list(engine);
    register_box_registry_add(engine);
    register_box_registry_delete(engine);
    register_box_compose_git_get(engine);
    register_box_compose_git_save(engine);
    register_box_compose_git_sync(engine);
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposeExtraHosts(pub Vec<String>);

impl<'de> serde::Deserialize<'de> for ComposeExtraHosts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ExtraHostsVisitor;
        impl<'de> serde::de::Visitor<'de> for ExtraHostsVisitor {
            type Value = ComposeExtraHosts;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of strings or a map of hostnames to IPs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut hosts = Vec::new();
                while let Some(elem) = seq.next_element::<String>()? {
                    hosts.push(elem);
                }
                Ok(ComposeExtraHosts(hosts))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut hosts = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    hosts.push(format!("{}:{}", k, v));
                }
                Ok(ComposeExtraHosts(hosts))
            }
        }
        deserializer.deserialize_any(ExtraHostsVisitor)
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct ComposeCommand(pub Vec<String>);

impl<'de> serde::Deserialize<'de> for ComposeCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CmdVisitor;
        impl<'de> serde::de::Visitor<'de> for CmdVisitor {
            type Value = ComposeCommand;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a sequence of strings")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(ComposeCommand(v.split_whitespace().map(|s| s.to_string()).collect()))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut cmd = Vec::new();
                while let Some(elem) = seq.next_element::<String>()? {
                    cmd.push(elem);
                }
                Ok(ComposeCommand(cmd))
            }
        }
        deserializer.deserialize_any(CmdVisitor)
    }
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
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    let v_str = match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => String::new(),
                        _ => v.to_string(),
                    };
                    env.insert(k, v_str);
                }
                Ok(ComposeEnvironment(env))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut env = HashMap::new();
                while let Some(item) = seq.next_element::<serde_json::Value>()? {
                    let item_str = match item {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    let parts: Vec<&str> = item_str.splitn(2, '=').collect();
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
                formatter.write_str("a sequence of strings, integers, or objects mapping ports")
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
                        serde_json::Value::Object(obj) => {
                            let target = obj.get("target")
                                .map(|v| match v {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => String::new(),
                                })
                                .unwrap_or_default();
                            
                            let published = obj.get("published")
                                .map(|v| match v {
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => String::new(),
                                });

                            let host_ip = obj.get("host_ip")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            let protocol = obj.get("protocol")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            if !target.is_empty() {
                                let mut port_str = String::new();
                                if let Some(ip) = host_ip {
                                    port_str.push_str(&format!("{}:", ip));
                                }
                                if let Some(pub_port) = published {
                                    port_str.push_str(&format!("{}:", pub_port));
                                }
                                port_str.push_str(&target);
                                if let Some(proto) = protocol {
                                    port_str.push_str(&format!("/{}", proto));
                                }
                                ports.push(port_str);
                            }
                        }
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
    pub env_file: Option<serde_yaml::Value>,
    pub volumes: Option<Vec<String>>,
    pub entrypoint: Option<ComposeCommand>,
    pub command: Option<ComposeCommand>,
    pub depends_on: Option<Vec<String>>,
    pub networks: Option<Vec<String>>,
    pub restart: Option<String>,
    pub healthcheck: Option<ComposeHealthCheck>,
    pub mem_limit: Option<String>,
    pub cpus: Option<f64>,
    pub oom_score_adj: Option<i32>,
    pub read_only: Option<bool>,
    pub network_mode: Option<String>,
    pub extra_hosts: Option<ComposeExtraHosts>,
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
    pub volumes: Option<HashMap<String, serde_yaml::Value>>,
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

fn load_env_file(compose_path: &str, env_file_val: &serde_yaml::Value) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let compose_path_buf = Path::new(compose_path);
    let parent_dir = compose_path_buf.parent().unwrap_or_else(|| Path::new("."));

    let files = match env_file_val {
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Sequence(seq) => {
            let mut v = Vec::new();
            for item in seq {
                if let Some(s) = item.as_str() {
                    v.push(s.to_string());
                }
            }
            v
        }
        _ => Vec::new(),
    };

    for file_name in files {
        let f_path = parent_dir.join(&file_name);
        if let Ok(content) = fs::read_to_string(f_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let k = parts[0].trim().to_string();
                    let v = parts[1].trim().to_string();
                    let v_clean = if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
                        if v.len() >= 2 {
                            v[1..v.len()-1].to_string()
                        } else {
                            v
                        }
                    } else {
                        v
                    };
                    env.insert(k, v_clean);
                }
            }
        }
    }

    env
}

fn inject_hosts_entries(
    data_dir: &str,
    container_id: &str,
    services: &HashMap<String, ComposeService>,
    current_name: &str,
) -> Result<(), String> {
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

    if let Some(svc) = services.get(current_name) {
        if let Some(ref eh) = svc.extra_hosts {
            for entry in &eh.0 {
                let parts: Vec<&str> = entry.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let hostname = parts[0].trim();
                    let ip = parts[1].trim();
                    entries.push(format!("{}\t{}", ip, hostname));
                }
            }
        }
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

    let compose_path_buf = Path::new(path);
    let project_name = compose_path_buf
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "default".to_string());

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
        let default_cmd = if !cache_dir.exists() {
            output.push_str(&format!("  ▶ Image {} not found locally. Pulling...\n", image));
            let rt = tokio::runtime::Handle::current();
            let pull_res = tokio::task::block_in_place(|| {
                rt.block_on(async { pull_image_rust(image).await })
            });
            match pull_res {
                Ok(cmd) => cmd,
                Err(e) => {
                    return Err(format!("Failed to pull image {}: {}", image, e));
                }
            }
        } else {
            get_image_default_cmd(image)
        };

        let container_name = svc.container_name.as_ref().unwrap_or(&name);

        let cont_p = container_dir(&data_dir, container_name);
        if cont_p.exists() {
            output.push_str(&format!("  ▶ Container '{}' already exists. Stopping and removing first...\n", container_name));
            let _ = container_stop(container_name);
            let _ = container_delete(container_name);
        }

        let cmd_args = match (&svc.entrypoint, &svc.command) {
            (Some(entrypoint), Some(command)) => {
                let mut cmd = entrypoint.0.clone();
                cmd.extend(command.0.clone());
                cmd
            }
            (Some(entrypoint), None) => {
                entrypoint.0.clone()
            }
            (None, Some(command)) => {
                command.0.clone()
            }
            (None, None) => {
                default_cmd
            }
        };

        let mut env = if let Some(ref e) = svc.environment {
            e.0.clone()
        } else {
            HashMap::new()
        };

        if let Some(ref env_file_val) = svc.env_file {
            let loaded_env = load_env_file(path, env_file_val);
            for (k, v) in loaded_env {
                env.entry(k).or_insert(v);
            }
        }

        let mut volumes = Vec::new();
        if let Some(ref vols) = svc.volumes {
            for v in vols {
                let parts: Vec<&str> = v.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let host_path = parts[0];
                    let container_path = parts[1];
                    let is_named_volume = !host_path.starts_with('/') && !host_path.starts_with('.') && !host_path.starts_with('~');
                    if is_named_volume {
                        let mut final_host_path = host_path.to_string();
                        let mut use_prefix = true;

                        if let Some(ref top_volumes) = cf.volumes {
                            if let Some(vol_config) = top_volumes.get(host_path) {
                                if let Some(map) = vol_config.as_mapping() {
                                    if let Some(ext_val) = map.get(&serde_yaml::Value::String("external".to_string())) {
                                        if ext_val.as_bool() == Some(true) {
                                            use_prefix = false;
                                        }
                                    }
                                    if let Some(name_val) = map.get(&serde_yaml::Value::String("name".to_string())) {
                                        if let Some(custom_name) = name_val.as_str() {
                                            final_host_path = custom_name.to_string();
                                            use_prefix = false;
                                        }
                                    }
                                }
                            }
                        }

                        if use_prefix {
                            volumes.push(format!("{}_{}:{}", project_name, final_host_path, container_path));
                        } else {
                            volumes.push(format!("{}:{}", final_host_path, container_path));
                        }
                    } else {
                        let resolved_path = if host_path.starts_with('.') {
                            if let Some(parent) = compose_path_buf.parent() {
                                let abs_path = parent.join(host_path);
                                if let Ok(canonical) = fs::canonicalize(&abs_path) {
                                    canonical.to_string_lossy().to_string()
                                } else {
                                    abs_path.to_string_lossy().to_string()
                                }
                            } else {
                                host_path.to_string()
                            }
                        } else {
                            host_path.to_string()
                        };
                        volumes.push(format!("{}:{}", resolved_path, container_path));
                    }
                } else {
                    volumes.push(v.clone());
                }
            }
        }

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
        let is_host_net = svc.network_mode.as_deref() == Some("host") || network_name == "host";
        container_create(
            container_name,
            image,
            cmd_args,
            env,
            "",
            volumes,
            ports,
            is_host_net,
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

        // Upgrade 2: Healthcheck support — wait for container health if specified
        if let Some(ref hc) = svc.healthcheck {
            output.push_str(&format!("  ▶ Waiting for healthcheck on '{}'...\n", container_name));
            let retries = hc.retries.unwrap_or(10);
            let mut healthy = false;
            let mut last_log = String::new();

            for attempt in 1..=retries {
                std::thread::sleep(std::time::Duration::from_secs(2));

                let hc_cmd = match &hc.test {
                    serde_yaml::Value::String(s) => vec!["sh".to_string(), "-c".to_string(), s.clone()],
                    serde_yaml::Value::Sequence(seq) => {
                        let mut vec = Vec::new();
                        for item in seq {
                            if let Some(s) = item.as_str() {
                                vec.push(s.to_string());
                            }
                        }
                        if vec.first().map(|s| s.as_str()) == Some("CMD-SHELL") {
                            vec.remove(0);
                            vec = vec!["sh".to_string(), "-c".to_string(), vec.join(" ")];
                        } else if vec.first().map(|s| s.as_str()) == Some("CMD") {
                            vec.remove(0);
                        }
                        vec
                    }
                    _ => Vec::new(),
                };

                if !hc_cmd.is_empty() {
                    let mut exec_args = vec!["exec", container_name];
                    let cmd_strs: Vec<&str> = hc_cmd.iter().map(|s| s.as_str()).collect();
                    exec_args.extend(cmd_strs);

                    let res = crate::slots::zeno_box::get_runc_bin();
                    let root = format!("{}/runc", get_data_dir());
                    let mut full_args = vec!["--root", &root];
                    full_args.extend(exec_args);

                    let hc_exec = std::process::Command::new(&res).args(&full_args).output();
                    if let Ok(out) = hc_exec {
                        if out.status.success() {
                            healthy = true;
                            output.push_str(&format!("  ✓ Healthcheck passed for '{}' on attempt {}/{}.\n", container_name, attempt, retries));
                            break;
                        } else {
                            last_log = String::from_utf8_lossy(&out.stderr).to_string();
                        }
                    }
                }
            }

            if !healthy {
                output.push_str(&format!("  ⚠ Warning: Healthcheck timed out for '{}' after {} attempts. (Last error: {})\n", container_name, retries, last_log.trim()));
            }
        }

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

    let containers = container_list_internal(&data_dir, false)?;
    
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
            let mut file_name = "docker-compose.yml".to_string();

            if node.value.is_some() {
                action = resolve_node_value(_engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                match child.name.as_str() {
                    "action" => action = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "yaml" => yaml = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "file" | "file_name" => file_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
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
            let compose_path = compose_dir.join(&file_name);

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
                    "save" => Ok("Saved successfully".to_string()),
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
            let mut file_name = "docker-compose.yml".to_string();
            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "file" | "file_name" => file_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
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
            let compose_path = compose_dir.join(&file_name);
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

fn register_box_compose_list_projects(engine: &mut Engine) {
    engine.register(
        "box.compose_list_projects",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose");
            
            let mut list = Vec::new();
            if let Ok(entries) = fs::read_dir(compose_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Ok(meta) = entry.metadata() {
                            if meta.is_dir() {
                                let mut map = HashMap::new();
                                map.insert("name".to_string(), Value::String(entry.file_name().to_string_lossy().into_owned()));
                                map.insert("is_dir".to_string(), Value::Bool(true));
                                list.push(Value::Map(map));
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List all compose projects".to_string(),
            example: "box.compose_list_projects { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_delete_project(engine: &mut Engine) {
    engine.register(
        "box.compose_delete_project",
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

            if project_name.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let success = if compose_dir.exists() && compose_dir.is_dir() {
                fs::remove_dir_all(compose_dir).is_ok()
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Delete compose project".to_string(),
            example: "box.compose_delete_project { project_name: 'test', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_registry_list(engine: &mut Engine) {
    engine.register(
        "box.registry_list",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let mut list = Vec::new();
            if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
                let p = home.join(".docker/config.json");
                if p.exists() {
                    if let Ok(content) = fs::read_to_string(&p) {
                        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(auths) = config.get("auths").and_then(|a| a.as_object()) {
                                for (key, _) in auths {
                                    list.push(Value::String(key.clone()));
                                }
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(list));
            Ok(())
        }),
        SlotMeta {
            description: "List Docker Registry logins".to_string(),
            example: "box.registry_list { as: $logins }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_registry_add(engine: &mut Engine) {
    engine.register(
        "box.registry_add",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut registry = String::new();
            let mut username = String::new();
            let mut password = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "registry" => registry = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "username" => username = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "password" => password = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if registry.is_empty() || username.is_empty() || password.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let home = match std::env::var("HOME").ok().map(PathBuf::from) {
                Some(h) => h,
                None => {
                    scope.set(&target, Value::Bool(false));
                    return Ok(());
                }
            };

            let docker_dir = home.join(".docker");
            let _ = fs::create_dir_all(&docker_dir);
            let p = docker_dir.join("config.json");

            let mut config = if p.exists() {
                fs::read_to_string(&p)
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .unwrap_or_else(|| serde_json::json!({ "auths": {} }))
            } else {
                serde_json::json!({ "auths": {} })
            };

            if config.get("auths").is_none() {
                if let Some(obj) = config.as_object_mut() {
                    obj.insert("auths".to_string(), serde_json::json!({}));
                }
            }

            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);

            if let Some(auths) = config.get_mut("auths").and_then(|a| a.as_object_mut()) {
                auths.insert(
                    registry.clone(),
                    serde_json::json!({
                        "auth": encoded
                    }),
                );
            }

            let success = if let Ok(pretty) = serde_json::to_string_pretty(&config) {
                fs::write(p, pretty).is_ok()
            } else {
                false
            };

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Add Docker Registry credentials".to_string(),
            example: "box.registry_add { registry: 'ghcr.io', username: 'foo', password: 'bar', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_registry_delete(engine: &mut Engine) {
    engine.register(
        "box.registry_delete",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut registry = String::new();
            for child in &node.children {
                match child.name.as_str() {
                    "registry" => registry = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if registry.is_empty() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let home = match std::env::var("HOME").ok().map(PathBuf::from) {
                Some(h) => h,
                None => {
                    scope.set(&target, Value::Bool(false));
                    return Ok(());
                }
            };

            let p = home.join(".docker/config.json");
            if !p.exists() {
                scope.set(&target, Value::Bool(false));
                return Ok(());
            }

            let mut success = false;
            if let Ok(content) = fs::read_to_string(&p) {
                if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(auths) = config.get_mut("auths").and_then(|a| a.as_object_mut()) {
                        if auths.remove(&registry).is_some() {
                            if let Ok(pretty) = serde_json::to_string_pretty(&config) {
                                success = fs::write(p, pretty).is_ok();
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::Bool(success));
            Ok(())
        }),
        SlotMeta {
            description: "Delete Docker Registry credentials".to_string(),
            example: "box.registry_delete { registry: 'ghcr.io', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_get(engine: &mut Engine) {
    engine.register(
        "box.compose_git_get",
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
            let git_config_path = Path::new(&data_dir).join("compose").join(&project_name).join(".zeno-git.json");

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
        SlotMeta {
            description: "Get Git settings for a Compose project".to_string(),
            example: "box.compose_git_get { project_name: 'my-app', as: $git }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_save(engine: &mut Engine) {
    engine.register(
        "box.compose_git_save",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "result".to_string();
            let mut project_name = String::new();
            let mut repo_url = String::new();
            let mut branch = "main".to_string();
            let mut webhook_token = String::new();

            for child in &node.children {
                match child.name.as_str() {
                    "project_name" => project_name = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "repo_url" => repo_url = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "branch" => branch = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "webhook_token" => webhook_token = resolve_node_value(_engine, child, scope).to_string_coerce(),
                    "as" => {
                        if let Some(ref val) = child.value {
                            target = val.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let data_dir = get_data_dir();
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let git_config_path = compose_dir.join(".zeno-git.json");

            let mut success = false;
            if fs::create_dir_all(&compose_dir).is_ok() {
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
        SlotMeta {
            description: "Save Git settings for a Compose project".to_string(),
            example: "box.compose_git_save { project_name: 'my-app', repo_url: 'https://...', branch: 'main', webhook_token: '...', as: $success }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_compose_git_sync(engine: &mut Engine) {
    engine.register(
        "box.compose_git_sync",
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
            let compose_dir = Path::new(&data_dir).join("compose").join(&project_name);
            let git_config_path = compose_dir.join(".zeno-git.json");

            let mut result = HashMap::new();
            result.insert("success".to_string(), Value::Bool(false));

            if !git_config_path.exists() {
                result.insert("stderr".to_string(), Value::String("No Git configuration found for this project".to_string()));
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

            if repo_url.is_empty() {
                result.insert("stderr".to_string(), Value::String("Git repository URL is empty".to_string()));
                scope.set(&target, Value::Map(result));
                return Ok(());
            }

            let volume_path = Path::new(&data_dir).join("volumes").join(format!("{}_app_data", project_name));
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
                    let compose_path = compose_dir.join("docker-compose.yml");
                    if compose_path.exists() {
                        match compose_up(&compose_path.to_string_lossy()) {
                            Ok(up_msg) => {
                                result.insert("success".to_string(), Value::Bool(true));
                                result.insert("stdout".to_string(), Value::String(format!("{}\n\n{}", stdout_msg, up_msg)));
                            }
                            Err(e) => {
                                result.insert("stderr".to_string(), Value::String(format!("Git pulled but compose restart failed: {}", e)));
                            }
                        }
                    } else {
                        result.insert("success".to_string(), Value::Bool(true));
                        result.insert("stdout".to_string(), Value::String(format!("{} (Note: No docker-compose.yml to restart)", stdout_msg)));
                    }
                }
                Err(e) => {
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Synchronize/pull Git repository files to project volume and restart compose containers".to_string(),
            example: "box.compose_git_sync { project_name: 'my-app', as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::common::parse_port_rule;
    use super::super::image::{get_docker_auth_for_registry, parse_www_authenticate};

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    fn test_compose_git_slots() {
        use zenocore::{Context, Scope};
        let _guard = TEST_MUTEX.lock().unwrap();
        
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_git_slots_{}", rand::random::<u32>()));
        fs::create_dir_all(&temp_dir).unwrap();
        
        let old_dir = std::env::var("ZENO_CONTAINER_DATA_DIR").ok();
        unsafe {
            std::env::set_var("ZENO_CONTAINER_DATA_DIR", &temp_dir);
        }

        let mut engine = Engine::new();
        register_box_compose_git_get(&mut engine);
        register_box_compose_git_save(&mut engine);
        
        let mut ctx = Context::new();
        let scope = Scope::new(None);
        
        // Save git settings
        let save_node = zenocore::parser::parse_string(
            r#"
            box.compose_git_save: {
                project_name: "test_git_project_name_123"
                repo_url: "https://github.com/user/repo.git"
                branch: "production"
                webhook_token: "secret123"
                as: $success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &save_node, &scope).unwrap();
        assert_eq!(scope.get("success"), Some(Value::Bool(true)));
        
        // Get git settings
        let get_node = zenocore::parser::parse_string(
            r#"
            box.compose_git_get: {
                project_name: "test_git_project_name_123"
                as: $git
            }
            "#,
            "test"
        ).unwrap();
        let scope2 = Scope::new(None);
        engine.execute(&mut ctx, &get_node, &scope2).unwrap();
        let git_val = scope2.get("git").unwrap();
        assert!(matches!(git_val, Value::Map(_)));
        if let Value::Map(ref map) = git_val {
            assert_eq!(map.get("repo_url").unwrap(), &Value::String("https://github.com/user/repo.git".to_string()));
            assert_eq!(map.get("branch").unwrap(), &Value::String("production".to_string()));
            assert_eq!(map.get("webhook_token").unwrap(), &Value::String("secret123".to_string()));
        }
        
        let _ = fs::remove_dir_all(&temp_dir);
        unsafe {
            if let Some(d) = old_dir {
                std::env::set_var("ZENO_CONTAINER_DATA_DIR", d);
            } else {
                std::env::remove_var("ZENO_CONTAINER_DATA_DIR");
            }
        }
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
        assert_eq!(ordered[0], "db");
        assert_eq!(ordered[1], "app");
        assert_eq!(ordered[2], "web");
    }

    #[test]
    fn test_parse_port_rule() {
        let r1 = parse_port_rule("80").unwrap();
        assert_eq!(r1.host_ip, None);
        assert_eq!(r1.host_port, "80");
        assert_eq!(r1.container_port, "80");
        assert_eq!(r1.protocol, "tcp");

        let r2 = parse_port_rule("8080:80").unwrap();
        assert_eq!(r2.host_ip, None);
        assert_eq!(r2.host_port, "8080");
        assert_eq!(r2.container_port, "80");
        assert_eq!(r2.protocol, "tcp");

        let r3 = parse_port_rule("127.0.0.1:8080:80").unwrap();
        assert_eq!(r3.host_ip, Some("127.0.0.1".to_string()));
        assert_eq!(r3.host_port, "8080");
        assert_eq!(r3.container_port, "80");
        assert_eq!(r3.protocol, "tcp");

        let r4 = parse_port_rule("127.0.0.1:8080:80/udp").unwrap();
        assert_eq!(r4.host_ip, Some("127.0.0.1".to_string()));
        assert_eq!(r4.host_port, "8080");
        assert_eq!(r4.container_port, "80");
        assert_eq!(r4.protocol, "udp");

        let r5 = parse_port_rule("80-82:80-82").unwrap();
        assert_eq!(r5.host_ip, None);
        assert_eq!(r5.host_port, "80-82");
        assert_eq!(r5.container_port, "80-82");
        assert_eq!(r5.protocol, "tcp");
    }

    #[test]
    fn test_compose_yaml_deserialization_advanced() {
        let yaml_content = r#"
version: '3.8'
services:
  web:
    image: nginx:latest
    entrypoint: /usr/bin/nginx
    command: ["-g", "daemon off;"]
    ports:
      - 80
      - "8080:80"
      - target: 9000
        published: 9001
        host_ip: 127.0.0.1
        protocol: udp
    environment:
      DEBUG: true
      PORT: 8080
      DB_HOST: "postgres"
    env_file:
      - .env
    extra_hosts:
      somehost: "10.0.0.1"
      otherhost: "10.0.0.2"
volumes:
  db_data: {}
"#;
        let cf: ComposeFile = serde_yaml::from_str(yaml_content).expect("Failed to parse advanced mock YAML");
        let web = cf.services.get("web").unwrap();
        
        assert_eq!(web.entrypoint.as_ref().unwrap().0, vec!["/usr/bin/nginx".to_string()]);
        assert_eq!(web.command.as_ref().unwrap().0, vec!["-g".to_string(), "daemon off;".to_string()]);

        let ports = &web.ports.as_ref().unwrap().0;
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0], "80");
        assert_eq!(ports[1], "8080:80");
        assert_eq!(ports[2], "127.0.0.1:9001:9000/udp");

        let env = &web.environment.as_ref().unwrap().0;
        assert_eq!(env.get("DEBUG").unwrap(), "true");
        assert_eq!(env.get("PORT").unwrap(), "8080");
        assert_eq!(env.get("DB_HOST").unwrap(), "postgres");

        let env_file = web.env_file.as_ref().unwrap();
        assert_eq!(env_file.as_sequence().unwrap()[0].as_str().unwrap(), ".env");

        let extra_hosts = &web.extra_hosts.as_ref().unwrap().0;
        assert!(extra_hosts.contains(&"somehost:10.0.0.1".to_string()));
        assert!(extra_hosts.contains(&"otherhost:10.0.0.2".to_string()));

        assert!(cf.volumes.as_ref().unwrap().contains_key("db_data"));
    }

    #[test]
    fn test_docker_config_and_registry_auth() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_docker_config_{}", rand::random::<u32>()));
        let docker_dir = temp_dir.join(".docker");
        fs::create_dir_all(&docker_dir).unwrap();
        
        let config_content = r#"{
            "auths": {
                "ghcr.io": {
                    "auth": "bXktdXNlcjpteS1wYXNzd29yZA=="
                },
                "https://index.docker.io/v1/": {
                    "auth": "ZG9ja2VyLXVzZXI6ZG9ja2VyLXBhc3N3b3Jk"
                }
            }
        }"#;
        fs::write(docker_dir.join("config.json"), config_content).unwrap();

        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &temp_dir);
        }

        let ghcr_auth = get_docker_auth_for_registry("https://ghcr.io");
        assert_eq!(ghcr_auth, Some(("my-user".to_string(), "my-password".to_string())));

        let docker_auth = get_docker_auth_for_registry("https://registry-1.docker.io");
        assert_eq!(docker_auth, Some(("docker-user".to_string(), "docker-password".to_string())));

        unsafe {
            if let Some(h) = old_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_www_authenticate() {
        let header = r#"Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:user/repo:pull""#;
        let parsed = parse_www_authenticate(header);
        assert_eq!(
            parsed,
            Some((
                "https://ghcr.io/token".to_string(),
                "ghcr.io".to_string(),
                "repository:user/repo:pull".to_string()
            ))
        );
    }

    #[test]
    fn test_registry_slots_engine() {
        use zenocore::{Context, Scope};
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!("zeno_test_registry_slots_{}", rand::random::<u32>()));
        let docker_dir = temp_dir.join(".docker");
        fs::create_dir_all(&docker_dir).unwrap();

        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("HOME", &temp_dir);
        }

        let mut engine = Engine::new();
        register_box_registry_list(&mut engine);
        register_box_registry_add(&mut engine);
        register_box_registry_delete(&mut engine);

        let mut ctx = Context::new();
        let scope = Scope::new(None);

        let node_add = zenocore::parser::parse_string(
            r#"
            box.registry_add: {
                registry: 'ghcr.io'
                username: 'test-user'
                password: 'test-password'
                as: $success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_add, &scope).unwrap();
        assert_eq!(scope.get("success"), Some(Value::Bool(true)));

        let node_list = zenocore::parser::parse_string(
            r#"
            box.registry_list: {
                as: $list
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_list, &scope).unwrap();
        
        if let Some(Value::List(lst)) = scope.get("list") {
            assert_eq!(lst.len(), 1);
            assert_eq!(lst[0], Value::String("ghcr.io".to_string()));
        } else {
            panic!("Expected list value");
        }

        let node_del = zenocore::parser::parse_string(
            r#"
            box.registry_delete: {
                registry: 'ghcr.io'
                as: $del_success
            }
            "#,
            "test"
        ).unwrap();
        engine.execute(&mut ctx, &node_del, &scope).unwrap();
        assert_eq!(scope.get("del_success"), Some(Value::Bool(true)));

        engine.execute(&mut ctx, &node_list, &scope).unwrap();
        if let Some(Value::List(lst)) = scope.get("list") {
            assert_eq!(lst.len(), 0);
        } else {
            panic!("Expected list value");
        }

        unsafe {
            if let Some(h) = old_home {
                std::env::set_var("HOME", h);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
