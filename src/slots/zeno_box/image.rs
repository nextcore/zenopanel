use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write};
use serde_json::json;
use zenocore::{Engine, SlotMeta, Value};
use crate::slots::resolve_node_value;

use super::common::{
    get_data_dir, container_dir, rootfs_dir, is_overlay_mounted,
    run_privileged_output, run_privileged_status, ImageRef, parse_image_ref
};

pub fn register(engine: &mut Engine) {
    register_box_pull(engine);
    register_box_images(engine);
    register_box_rmi(engine);
    register_box_prune(engine);
}

fn get_docker_auth_for_registry(registry_url: &str) -> Option<(String, String)> {
    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let paths: [PathBuf; 2] = [
        home.join(".docker/config.json"),
        PathBuf::from("/root/.docker/config.json"),
    ];

    let host = registry_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    for p in &paths {
        if p.exists() {
            if let Ok(content) = fs::read_to_string(p) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(auths) = config.get("auths").and_then(|a| a.as_object()) {
                        for (key, val) in auths {
                            let key_clean = key.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
                            let host_clean = host.trim_end_matches('/');
                            let is_match = key_clean == host_clean 
                                || (host_clean == "registry-1.docker.io" && key_clean == "index.docker.io/v1");
                            
                            if is_match {
                                if let Some(auth_str) = val.get("auth").and_then(|a| a.as_str()) {
                                    use base64::{Engine as _, engine::general_purpose::STANDARD};
                                    if let Ok(decoded) = STANDARD.decode(auth_str) {
                                        if let Ok(decoded_str) = String::from_utf8(decoded) {
                                            let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
                                            if parts.len() == 2 {
                                                return Some((parts[0].to_string(), parts[1].to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn parse_www_authenticate(header_val: &str) -> Option<(String, String, String)> {
    if !header_val.starts_with("Bearer ") {
        return None;
    }
    let params = &header_val[7..];
    let mut realm = None;
    let mut service = None;
    let mut scope = None;

    for part in params.split(',') {
        let kv: Vec<&str> = part.splitn(2, '=').collect();
        if kv.len() == 2 {
            let k = kv[0].trim();
            let v = kv[1].trim().trim_matches('"');
            if k == "realm" {
                realm = Some(v.to_string());
            } else if k == "service" {
                service = Some(v.to_string());
            } else if k == "scope" {
                scope = Some(v.to_string());
            }
        }
    }

    if let (Some(r), Some(s)) = (realm, service) {
        Some((r, s, scope.unwrap_or_default()))
    } else {
        None
    }
}

fn apply_auth_header(req: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    if token.is_empty() {
        req
    } else if token.starts_with("Bearer ") || token.starts_with("Basic ") {
        req.header("Authorization", token)
    } else {
        req.header("Authorization", format!("Bearer {}", token))
    }
}

async fn get_registry_token(client: &reqwest::Client, img: &ImageRef) -> Result<String, String> {
    let host = img.registry.trim_start_matches("https://").trim_start_matches("http://");
    let credentials = get_docker_auth_for_registry(&img.registry);

    if host == "registry-1.docker.io" {
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope=repository:{}:pull",
            img.repository
        );
        let mut req = client.get(&auth_url);
        if let Some((ref username, ref password)) = credentials {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);
            req = req.header("Authorization", format!("Basic {}", encoded));
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            if let Some(tok) = json.get("token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
            if let Some(tok) = json.get("access_token").and_then(|v| v.as_str()) {
                return Ok(tok.to_string());
            }
        }
        
        if let Some((ref username, ref password)) = credentials {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let auth_val = format!("{}:{}", username, password);
            let encoded = STANDARD.encode(auth_val);
            return Ok(format!("Basic {}", encoded));
        }
        
        return Ok(String::new());
    }

    let v2_url = format!("{}/v2/", img.registry);
    let mut req = client.get(&v2_url);
    if let Some((ref username, ref password)) = credentials {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let auth_val = format!("{}:{}", username, password);
        let encoded = STANDARD.encode(auth_val);
        req = req.header("Authorization", format!("Basic {}", encoded));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if resp.status() == 401 {
        if let Some(auth_header) = resp.headers().get("www-authenticate").and_then(|h| h.to_str().ok()) {
            if let Some((realm, service, scope)) = parse_www_authenticate(auth_header) {
                let final_scope = if scope.is_empty() {
                    format!("repository:{}:pull", img.repository)
                } else {
                    scope
                };
                let mut token_req = client.get(&realm)
                    .query(&[("service", &service), ("scope", &final_scope)]);
                
                if let Some((ref username, ref password)) = credentials {
                    use base64::{Engine as _, engine::general_purpose::STANDARD};
                    let auth_val = format!("{}:{}", username, password);
                    let encoded = STANDARD.encode(auth_val);
                    token_req = token_req.header("Authorization", format!("Basic {}", encoded));
                }
                
                let token_resp = token_req.send().await.map_err(|e| e.to_string())?;
                if token_resp.status().is_success() {
                    let json: serde_json::Value = token_resp.json().await.map_err(|e| e.to_string())?;
                    if let Some(tok) = json.get("token").and_then(|v| v.as_str()) {
                        return Ok(tok.to_string());
                    }
                    if let Some(tok) = json.get("access_token").and_then(|v| v.as_str()) {
                        return Ok(tok.to_string());
                    }
                }
            }
        }
    }

    if let Some((ref username, ref password)) = credentials {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let auth_val = format!("{}:{}", username, password);
        let encoded = STANDARD.encode(auth_val);
        return Ok(format!("Basic {}", encoded));
    }

    Ok(String::new())
}

pub(crate) async fn pull_image_rust(image: &str) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let img_ref = parse_image_ref(image);
    let token = get_registry_token(&client, &img_ref).await?;

    let data_dir = get_data_dir();
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    
    let image_cache_dir = format!("{}/images/{}", data_dir, cache_dir_name);
    let layers_cache_dir = format!("{}/images/layers", data_dir);

    fs::create_dir_all(&image_cache_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&layers_cache_dir).map_err(|e| e.to_string())?;

    let manifest_url = format!("{}/v2/{}/manifests/{}", img_ref.registry, img_ref.repository, img_ref.tag);
    let mut req = client.get(&manifest_url)
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json");
    req = apply_auth_header(req, &token);
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch manifest: status {}", resp.status()));
    }

    let headers = resp.headers().clone();
    let content_type = headers.get("Content-Type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let body_bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let mut manifest_json: serde_json::Value = serde_json::from_slice(&body_bytes).map_err(|e| e.to_string())?;

    if content_type.contains("manifest.list") || content_type.contains("image.index") {
        let mut selected_digest = None;
        if let Some(manifests) = manifest_json.get("manifests").and_then(|m| m.as_array()) {
            for m in manifests {
                let platform = m.get("platform");
                let os = platform.and_then(|p| p.get("os")).and_then(|o| o.as_str()).unwrap_or("");
                let arch = platform.and_then(|p| p.get("architecture")).and_then(|a| a.as_str()).unwrap_or("");
                if os == "linux" && arch == "amd64" {
                    selected_digest = m.get("digest").and_then(|d| d.as_str()).map(|s| s.to_string());
                    break;
                }
            }
            if selected_digest.is_none() && !manifests.is_empty() {
                selected_digest = manifests[0].get("digest").and_then(|d| d.as_str()).map(|s| s.to_string());
            }
        }

        let digest = selected_digest.ok_or_else(|| "No matching manifest in list".to_string())?;
        
        let manifest_by_digest_url = format!("{}/v2/{}/manifests/{}", img_ref.registry, img_ref.repository, digest);
        let mut req2 = client.get(&manifest_by_digest_url).header("Accept", "application/vnd.docker.distribution.manifest.v2+json");
        req2 = apply_auth_header(req2, &token);
        let resp2 = req2.send().await.map_err(|e| e.to_string())?;
        if !resp2.status().is_success() {
            return Err(format!("Failed to fetch resolved manifest: status {}", resp2.status()));
        }
        let body2 = resp2.bytes().await.map_err(|e| e.to_string())?;
        manifest_json = serde_json::from_slice(&body2).map_err(|e| e.to_string())?;
    }

    let config_digest = manifest_json.get("config")
        .and_then(|c| c.get("digest"))
        .and_then(|d| d.as_str())
        .ok_or_else(|| "Missing config digest in manifest".to_string())?;

    let config_url = format!("{}/v2/{}/blobs/{}", img_ref.registry, img_ref.repository, config_digest);
    let mut req_cfg = client.get(&config_url);
    req_cfg = apply_auth_header(req_cfg, &token);
    let resp_cfg = req_cfg.send().await.map_err(|e| e.to_string())?;
    let config_bytes = resp_cfg.bytes().await.map_err(|e| e.to_string())?;
    let image_config_json: serde_json::Value = serde_json::from_slice(&config_bytes).map_err(|e| e.to_string())?;

    let layers = manifest_json.get("layers")
        .and_then(|l| l.as_array())
        .ok_or_else(|| "Missing layers in manifest".to_string())?;

    let mut layer_digests = Vec::new();
    for (_i, layer) in layers.iter().enumerate() {
        let digest = layer.get("digest").and_then(|d| d.as_str()).ok_or_else(|| "Missing layer digest".to_string())?;
        let digest_clean = digest.trim_start_matches("sha256:");
        layer_digests.push(digest_clean.to_string());

        let layer_dir = format!("{}/{}", layers_cache_dir, digest_clean);
        let layer_rootfs = format!("{}/rootfs", layer_dir);
        let tar_gz_path = format!("{}/{}.tar.gz", layer_dir, digest_clean);

        fs::create_dir_all(&layer_dir).map_err(|e| e.to_string())?;

        if !Path::new(&layer_rootfs).exists() {
            if !Path::new(&tar_gz_path).exists() {
                let blob_url = format!("{}/v2/{}/blobs/{}", img_ref.registry, img_ref.repository, digest);
                let mut req_blob = client.get(&blob_url);
                req_blob = apply_auth_header(req_blob, &token);
                let resp_blob = req_blob.send().await.map_err(|e| e.to_string())?;
                if !resp_blob.status().is_success() {
                    return Err(format!("Layer download failed: status {}", resp_blob.status()));
                }
                let blob_bytes = resp_blob.bytes().await.map_err(|e| e.to_string())?;
                fs::write(&tar_gz_path, &blob_bytes).map_err(|e| e.to_string())?;
            }

            fs::create_dir_all(&layer_rootfs).map_err(|e| e.to_string())?;
            let tar_gz = File::open(&tar_gz_path).map_err(|e| e.to_string())?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&layer_rootfs).map_err(|e| e.to_string())?;
            
            // Delete compressed .tar.gz archive to prevent double storage disk usage
            let _ = fs::remove_file(&tar_gz_path);
        }
    }

    fs::write(
        format!("{}/layers.json", image_cache_dir),
        serde_json::to_string(&layer_digests).unwrap()
    ).map_err(|e| e.to_string())?;

    fs::write(
        format!("{}/image-config.json", image_cache_dir),
        serde_json::to_string_pretty(&image_config_json).unwrap()
    ).map_err(|e| e.to_string())?;

    let mut final_cmd = Vec::new();
    if let Some(entrypoint) = image_config_json.get("config").and_then(|c| c.get("Entrypoint")).and_then(|e| e.as_array()) {
        for val in entrypoint {
            if let Some(s) = val.as_str() {
                final_cmd.push(s.to_string());
            }
        }
    }
    if let Some(cmd) = image_config_json.get("config").and_then(|c| c.get("Cmd")).and_then(|e| e.as_array()) {
        for val in cmd {
            if let Some(s) = val.as_str() {
                final_cmd.push(s.to_string());
            }
        }
    }

    // Resolve relative script names like 'docker-entrypoint.sh' to absolute paths to prevent OCI execution errors
    if !final_cmd.is_empty() && final_cmd[0] == "docker-entrypoint.sh" {
        final_cmd[0] = "/usr/local/bin/docker-entrypoint.sh".to_string();
    }

    Ok(final_cmd)
}

pub(crate) fn get_image_default_cmd(image: &str) -> Vec<String> {
    let data_dir = get_data_dir();
    let img_ref = parse_image_ref(image);
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    let cache_dir = Path::new(&data_dir).join("images").join(&cache_dir_name);
    let config_path = cache_dir.join("image-config.json");
    let mut default_cmd = Vec::new();
    if config_path.exists() {
        if let Ok(file) = File::open(&config_path) {
            if let Ok(image_config_json) = serde_json::from_reader::<_, serde_json::Value>(file) {
                if let Some(entrypoint) = image_config_json.get("config").and_then(|c| c.get("Entrypoint")).and_then(|e| e.as_array()) {
                    for val in entrypoint {
                        if let Some(s) = val.as_str() {
                            default_cmd.push(s.to_string());
                        }
                    }
                }
                if let Some(cmd) = image_config_json.get("config").and_then(|c| c.get("Cmd")).and_then(|e| e.as_array()) {
                    for val in cmd {
                        if let Some(s) = val.as_str() {
                            default_cmd.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    if !default_cmd.is_empty() && default_cmd[0] == "docker-entrypoint.sh" {
        default_cmd[0] = "/usr/local/bin/docker-entrypoint.sh".to_string();
    }

    default_cmd
}

fn resolve_vfs_clashes(src_dir: &std::path::Path, dst_dir: &std::path::Path) -> std::io::Result<()> {
    if !src_dir.exists() {
        return Ok(());
    }
    
    fn walk_and_resolve(base_src: &std::path::Path, current_src: &std::path::Path, dst_dir: &std::path::Path) -> std::io::Result<()> {
        let entries = std::fs::read_dir(current_src)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let rel_path = path.strip_prefix(base_src)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let target_path = dst_dir.join(rel_path);

            if target_path.symlink_metadata().is_ok() {
                let src_meta = entry.path().symlink_metadata()?;
                let dst_meta = target_path.symlink_metadata()?;

                let src_is_dir = src_meta.is_dir();
                let dst_is_dir = dst_meta.is_dir();

                if !src_is_dir || !dst_is_dir {
                    if dst_is_dir {
                        std::fs::remove_dir_all(&target_path)?;
                    } else {
                        std::fs::remove_file(&target_path)?;
                    }
                }
            }

            if entry.file_type()?.is_dir() {
                walk_and_resolve(base_src, &path, dst_dir)?;
            }
        }
        Ok(())
    }

    walk_and_resolve(src_dir, src_dir, dst_dir)
}

fn process_whiteouts(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with(".wh.") {
                    if file_name == ".wh..wh..opq" {
                        let _ = fs::remove_file(&path);
                    } else {
                        let target_name = file_name.trim_start_matches(".wh.");
                        let target_path = dir.join(target_name);
                        if let Ok(meta) = target_path.symlink_metadata() {
                            if meta.is_dir() {
                                let _ = fs::remove_dir_all(&target_path);
                            } else {
                                let _ = fs::remove_file(&target_path);
                            }
                        }
                        let _ = fs::remove_file(&path);
                    }
                } else if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() && !file_type.is_symlink() {
                        let _ = process_whiteouts(&path);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn mount_overlayfs(image: &str, data_dir: &str, id: &str) -> Result<(), String> {
    let dst_rootfs = rootfs_dir(data_dir, id);

    if is_overlay_mounted(&dst_rootfs.to_string_lossy()) {
        return Ok(());
    }

    let img_ref = parse_image_ref(image);
    let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
        .replace('/', "_")
        .replace(':', "_");
    let image_cache_dir = Path::new(data_dir).join("images").join(&cache_dir_name);

    let layers_json_path = image_cache_dir.join("layers.json");
    if !layers_json_path.exists() {
        return Err(format!("Image metadata layers.json not found for {}", image));
    }

    let file = File::open(&layers_json_path).map_err(|e| e.to_string())?;
    let layers: Vec<String> = serde_json::from_reader(file).map_err(|e| e.to_string())?;

    let mut lowerdirs = Vec::new();
    let layers_dir = Path::new(data_dir).join("images").join("layers");
    for layer in layers.iter().rev() {
        lowerdirs.push(layers_dir.join(layer).join("rootfs").to_string_lossy().to_string());
    }
    let lowerdir_str = lowerdirs.join(":");

    let cont_dir = container_dir(data_dir, id);
    let upperdir = cont_dir.join("diff");
    let workdir = cont_dir.join("work");

    fs::create_dir_all(&upperdir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&workdir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dst_rootfs).map_err(|e| e.to_string())?;

    let opts = format!("lowerdir={},upperdir={},workdir={}", lowerdir_str, upperdir.to_string_lossy(), workdir.to_string_lossy());
    let dst_rootfs_str = dst_rootfs.to_string_lossy().to_string();

    let mount_output = run_privileged_output("mount", &["-t", "overlay", "overlay", "-o", &opts, &dst_rootfs_str])
        .map_err(|e| format!("Failed to run mount command: {}", e))?;

    if !mount_output.status.success() {
        let mount_err_msg = String::from_utf8_lossy(&mount_output.stderr).trim().to_string();

        for layer in &layers {
            let src_rootfs = layers_dir.join(layer).join("rootfs");
            if src_rootfs.exists() {
                if let Err(e) = resolve_vfs_clashes(&src_rootfs, &dst_rootfs) {
                    return Err(format!("VFS fallback clash resolution failed for layer {}: {}", layer, e));
                }

                let src_str = format!("{}/.", src_rootfs.to_string_lossy());
                let dst_str = dst_rootfs.to_string_lossy().to_string();
                let cp_res = run_privileged_output("cp", &["-a", &src_str, &dst_str]);
                
                let (success, cp_err_msg) = match cp_res {
                    Ok(out) if out.status.success() => (true, String::new()),
                    Ok(out) => (false, String::from_utf8_lossy(&out.stderr).trim().to_string()),
                    Err(e) => (false, e.to_string()),
                };

                let (success, final_err_msg) = if !success {
                    match Command::new("cp")
                        .args(&["-a", &src_str, &dst_str])
                        .output() 
                    {
                        Ok(out) if out.status.success() => (true, String::new()),
                        Ok(out) => (false, String::from_utf8_lossy(&out.stderr).trim().to_string()),
                        Err(e) => (false, e.to_string()),
                    }
                } else {
                    (success, cp_err_msg)
                };

                if !success {
                    return Err(format!(
                        "Overlay mount failed ({}), and VFS copy fallback failed for layer {}: {}",
                        mount_err_msg,
                        layer,
                        final_err_msg
                     ));
                }
            }
        }
        let _ = process_whiteouts(&dst_rootfs);
    }

    Ok(())
}

pub(crate) fn prune_unused_layers() -> io::Result<()> {
    let data_dir = get_data_dir();
    let images_dir = Path::new(&data_dir).join("images");
    if !images_dir.exists() {
        return Ok(());
    }

    let mut used_layers = std::collections::HashSet::new();

    if let Ok(entries) = fs::read_dir(&images_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && entry.file_name() != "layers" {
                let layers_json_path = path.join("layers.json");
                if layers_json_path.exists() {
                    if let Ok(file) = File::open(&layers_json_path) {
                        if let Ok(layers) = serde_json::from_reader::<_, Vec<String>>(file) {
                            for layer in layers {
                                used_layers.insert(layer);
                            }
                        }
                    }
                }
            }
        }
    }

    let layers_dir = images_dir.join("layers");
    if layers_dir.exists() {
        if let Ok(entries) = fs::read_dir(&layers_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let layer_name = entry.file_name().to_string_lossy().to_string();
                    if !used_layers.contains(&layer_name) {
                        let _ = fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    Ok(())
}

fn register_box_pull(engine: &mut Engine) {
    engine.register(
        "box.pull",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "pull_result".to_string();

            if node.value.is_some() {
                let resolved = resolve_node_value(_engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                if child.name == "image" {
                    image = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let rt = tokio::runtime::Handle::current();
            let res = tokio::task::block_in_place(|| {
                rt.block_on(async { pull_image_rust(&image).await })
            });

            let mut result = HashMap::new();
            match res {
                Ok(_cmd) => {
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String("Image pulled successfully".to_string()));
                    result.insert("stderr".to_string(), Value::String(String::new()));
                }
                Err(e) => {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stdout".to_string(), Value::String(String::new()));
                    result.insert("stderr".to_string(), Value::String(e));
                }
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Pull container image natively in Rust".to_string(),
            example: "box.pull: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_images(engine: &mut Engine) {
    engine.register(
        "box.images",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "images".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }

            let data_dir = get_data_dir();
            let images_dir = Path::new(&data_dir).join("images");
            let mut images = Vec::new();
            if let Ok(entries) = fs::read_dir(images_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if entry.path().is_dir() && entry.file_name() != "layers" {
                            let name = entry.file_name().to_string_lossy().to_string()
                                .replace('_', "/");
                            if let Some(idx) = name.rfind('/') {
                                let (repo, tag) = name.split_at(idx);
                                let tag_clean = tag.trim_start_matches('/');
                                images.push(Value::String(format!("{}:{}", repo, tag_clean)));
                            }
                        }
                    }
                }
            }

            scope.set(&target, Value::List(images));
            Ok(())
        }),
        SlotMeta {
            description: "List cached container images".to_string(),
            example: "box.images { as: $images }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_prune(engine: &mut Engine) {
    engine.register(
        "box.prune",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "prune_result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let mut result = HashMap::new();
            match prune_unused_layers() {
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
            description: "Prune unused OCI image layers".to_string(),
            example: "box.prune: { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}

fn register_box_rmi(engine: &mut Engine) {
    engine.register(
        "box.rmi",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut image = String::new();
            let mut target = "rmi_result".to_string();

            if node.value.is_some() {
                let resolved = resolve_node_value(_engine, node, scope);
                let val_str = resolved.to_string_coerce();
                if !val_str.is_empty() && !val_str.starts_with('$') {
                    image = val_str;
                }
            }

            for child in &node.children {
                if child.name == "image" {
                    image = resolve_node_value(_engine, child, scope).to_string_coerce();
                } else if child.name == "as" {
                    if let Some(ref v) = child.value {
                        target = v.trim_start_matches('$').to_string();
                    }
                }
            }

            let img_ref = parse_image_ref(&image);
            let cache_dir_name = format!("{}_{}", img_ref.repository, img_ref.tag)
                .replace('/', "_")
                .replace(':', "_");
            let data_dir = get_data_dir();
            let cache_dir = Path::new(&data_dir).join("images").join(cache_dir_name);

            let mut result = HashMap::new();
            if cache_dir.exists() {
                if let Err(e) = fs::remove_dir_all(cache_dir) {
                    result.insert("success".to_string(), Value::Bool(false));
                    result.insert("stderr".to_string(), Value::String(e.to_string()));
                } else {
                    let _ = prune_unused_layers();
                    result.insert("success".to_string(), Value::Bool(true));
                    result.insert("stdout".to_string(), Value::String("Image removed".to_string()));
                }
            } else {
                result.insert("success".to_string(), Value::Bool(false));
                result.insert("stderr".to_string(), Value::String("Image not found".to_string()));
            }

            scope.set(&target, Value::Map(result));
            Ok(())
        }),
        SlotMeta {
            description: "Remove a cached image".to_string(),
            example: "box.rmi: 'nginx:alpine' { as: $result }".to_string(),
            inputs: HashMap::new(),
            required_blocks: Vec::new(),
            value_type: "".to_string(),
        }
    );
}
