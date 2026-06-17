use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use super::resolve_node_value;
use std::sync::Arc;
use std::collections::HashMap;

fn zip_dir_recursive(
    src_dir: &std::path::Path,
    current_dir: &std::path::Path,
    zip: &mut zip::ZipWriter<std::fs::File>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            zip_dir_recursive(src_dir, &path, zip)?;
        } else {
            let name = path.strip_prefix(src_dir)?;
            zip.start_file(name.to_string_lossy(), zip::write::FileOptions::default())?;
            let mut f = std::fs::File::open(&path)?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

fn zip_file(src_file: &std::path::Path, zip: &mut zip::ZipWriter<std::fs::File>) -> Result<(), Box<dyn std::error::Error>> {
    let name = src_file.file_name().ok_or("Invalid file name")?;
    zip.start_file(name.to_string_lossy(), zip::write::FileOptions::default())?;
    let mut f = std::fs::File::open(src_file)?;
    std::io::copy(&mut f, zip)?;
    Ok(())
}

pub fn register(engine: &mut Engine) {
    engine.register(
        "io.file.archive",
        Arc::new(|engine, _ctx, node, scope| {
            let mut source = "".to_string();
            let mut dest = "".to_string();
            let mut target = "archive_result".to_string();

            if node.value.is_some() {
                source = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" || child.name == "src" {
                    source = val.to_string_coerce();
                } else if child.name == "dest" || child.name == "dst" {
                    dest = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if source.is_empty() || dest.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.archive: both source (path) and dest paths are required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.archive".to_string()),
                });
            }

            let src_path = std::path::Path::new(&source);
            let dst_path = std::path::Path::new(&dest);

            if let Some(parent) = dst_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.archive failed to create target parent dir: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.archive".to_string()),
                        }
                    })?;
                }
            }

            let is_tar_gz = dest.ends_with(".tar.gz") || dest.ends_with(".tgz");

            if is_tar_gz {
                let file = std::fs::File::create(&dst_path).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed to create destination file: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;

                let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
                let mut tar = tar::Builder::new(enc);

                let res = if src_path.is_dir() {
                    tar.append_dir_all(
                        src_path.file_name().ok_or("Invalid directory name").unwrap_or(std::ffi::OsStr::new("")),
                        src_path,
                    )
                } else {
                    match std::fs::File::open(src_path) {
                        Ok(mut f) => {
                            let name = src_path.file_name().ok_or("Invalid file name").unwrap_or(std::ffi::OsStr::new(""));
                            tar.append_file(name, &mut f)
                        }
                        Err(e) => Err(e)
                    }
                };

                res.map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;

                tar.finish().map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed to finalize tar.gz: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;
            } else {
                let file = std::fs::File::create(&dst_path).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed to create destination file: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;

                let mut zip = zip::ZipWriter::new(file);

                let res = if src_path.is_dir() {
                    zip_dir_recursive(src_path, src_path, &mut zip)
                } else {
                    zip_file(src_path, &mut zip)
                };

                res.map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;

                zip.finish().map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.archive failed to finalize zip: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.archive".to_string()),
                    }
                })?;
            }

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.extract",
        Arc::new(|engine, _ctx, node, scope| {
            let mut source = "".to_string();
            let mut dest = "".to_string();
            let mut target = "extract_result".to_string();

            if node.value.is_some() {
                source = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" || child.name == "src" {
                    source = val.to_string_coerce();
                } else if child.name == "dest" || child.name == "dst" {
                    dest = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            if source.is_empty() || dest.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.extract: both source (path) and dest paths are required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.extract".to_string()),
                });
            }

            let file = std::fs::File::open(&source).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.extract failed to open source file: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.extract".to_string()),
                }
            })?;

            let target_dir = std::path::Path::new(&dest);
            if !target_dir.exists() {
                std::fs::create_dir_all(target_dir).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.extract failed to create target directory: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.extract".to_string()),
                    }
                })?;
            }

            let is_tar_gz = source.ends_with(".tar.gz") || source.ends_with(".tgz");

            if is_tar_gz {
                let tar_file = flate2::read::GzDecoder::new(file);
                let mut archive = tar::Archive::new(tar_file);

                archive.unpack(target_dir).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.extract failed to unpack tar.gz: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.extract".to_string()),
                    }
                })?;
            } else {
                let mut archive = zip::ZipArchive::new(file).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.extract failed to read zip archive: {}", e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.extract".to_string()),
                    }
                })?;

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.extract failed to read zip entry: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.extract".to_string()),
                        }
                    })?;

                    let outpath = match file.enclosed_name() {
                        Some(path) => target_dir.join(path),
                        None => continue,
                    };

                    if (*file.name()).ends_with('/') {
                        std::fs::create_dir_all(&outpath).map_err(|e| {
                            Diagnostic {
                                r#type: "error".to_string(),
                                message: format!("io.file.extract failed to create entry directory: {}", e),
                                filename: node.filename.clone(),
                                line: node.line,
                                col: node.col,
                                slot: Some("io.file.extract".to_string()),
                            }
                        })?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            if !p.exists() {
                                std::fs::create_dir_all(&p).map_err(|e| {
                                    Diagnostic {
                                        r#type: "error".to_string(),
                                        message: format!("io.file.extract failed to create entry parent directory: {}", e),
                                        filename: node.filename.clone(),
                                        line: node.line,
                                        col: node.col,
                                        slot: Some("io.file.extract".to_string()),
                                    }
                                })?;
                            }
                        }
                        let mut outfile = std::fs::File::create(&outpath).map_err(|e| {
                            Diagnostic {
                                r#type: "error".to_string(),
                                message: format!("io.file.extract failed to create entry file: {}", e),
                                filename: node.filename.clone(),
                                line: node.line,
                                col: node.col,
                                slot: Some("io.file.extract".to_string()),
                            }
                        })?;
                        std::io::copy(&mut file, &mut outfile).map_err(|e| {
                            Diagnostic {
                                r#type: "error".to_string(),
                                message: format!("io.file.extract failed to copy file contents: {}", e),
                                filename: node.filename.clone(),
                                line: node.line,
                                col: node.col,
                                slot: Some("io.file.extract".to_string()),
                            }
                        })?;
                    }

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Some(mode) = file.unix_mode() {
                            std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
                        }
                    }
                }
            }

            scope.set(&target, Value::Bool(true));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.read",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = String::new();
            let mut target = "file_content".to_string();

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

            let content = std::fs::read_to_string(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.read failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.read".to_string()),
                }
            })?;

            scope.set(&target, Value::String(content));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.write",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = String::new();
            let mut content = String::new();
            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "content" {
                    content = val.to_string_coerce();
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.write: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.write".to_string()),
                });
            }

            if std::env::var("APP_ENV").unwrap_or_default() != "development" {
                let path_lower = path.to_lowercase();
                if path_lower.ends_with(".zl") || path_lower.ends_with(".rs") || path_lower.ends_with(".env") || path_lower.contains(".git") {
                    return Err(Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("security violation: modifying sensitive file '{}' is restricted in production", path),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.write".to_string()),
                    });
                }
            }

            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            std::fs::write(&path, content).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.file.write failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.write".to_string()),
                }
            })?;
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.dir.create",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = resolve_node_value(engine, node, scope).to_string_coerce();
            if path.is_empty() {
                for child in &node.children {
                    if child.name == "path" {
                        path = engine.resolve_shorthand_value(child, scope).to_string_coerce();
                    }
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.dir.create: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.dir.create".to_string()),
                });
            }

            std::fs::create_dir_all(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("io.dir.create failed: {}", e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.dir.create".to_string()),
                }
            })?;
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.delete",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = resolve_node_value(engine, node, scope).to_string_coerce();
            if path.is_empty() {
                for child in &node.children {
                    if child.name == "path" {
                        path = engine.resolve_shorthand_value(child, scope).to_string_coerce();
                    }
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.delete: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.delete".to_string()),
                });
            }

            let p = std::path::Path::new(&path);
            if p.exists() {
                if p.is_dir() {
                    std::fs::remove_dir_all(p).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.delete failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.delete".to_string()),
                        }
                    })?;
                } else {
                    std::fs::remove_file(p).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.delete failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.delete".to_string()),
                        }
                    })?;
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "io.file.chmod",
        Arc::new(|engine, _ctx, node, scope| {
            let mut path = String::new();
            let mut mode_str = String::new();
            let mut mode_int: Option<i64> = None;
            let mut recursive = false;

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "path" {
                    path = val.to_string_coerce();
                } else if child.name == "mode" {
                    match val {
                        Value::Int(i) => mode_int = Some(i),
                        Value::String(s) => mode_str = s,
                        _ => mode_str = val.to_string_coerce(),
                    }
                } else if child.name == "recursive" {
                    recursive = val.to_bool();
                }
            }

            if path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "io.file.chmod: path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("io.file.chmod".to_string()),
                });
            }

            let mode = if let Some(mi) = mode_int {
                u32::from_str_radix(&mi.to_string(), 8).unwrap_or(mi as u32)
            } else {
                let clean_mode = mode_str.trim().trim_start_matches("0o");
                u32::from_str_radix(clean_mode, 8).map_err(|e| {
                    Diagnostic {
                        r#type: "error".to_string(),
                        message: format!("io.file.chmod: invalid mode '{}': {}", mode_str, e),
                        filename: node.filename.clone(),
                        line: node.line,
                        col: node.col,
                        slot: Some("io.file.chmod".to_string()),
                    }
                })?
            };

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(mode);
                let path_obj = std::path::Path::new(&path);

                if recursive && path_obj.is_dir() {
                    fn set_perm_recursive(dir: &std::path::Path, perm: &std::fs::Permissions) -> std::io::Result<()> {
                        std::fs::set_permissions(dir, perm.clone())?;
                        for entry in std::fs::read_dir(dir)? {
                            let entry = entry?;
                            let entry_path = entry.path();
                            if entry_path.is_dir() {
                                set_perm_recursive(&entry_path, perm)?;
                            } else {
                                std::fs::set_permissions(&entry_path, perm.clone())?;
                            }
                        }
                        Ok(())
                    }
                    set_perm_recursive(path_obj, &permissions).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.chmod recursive failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.chmod".to_string()),
                        }
                    })?;
                } else {
                    std::fs::set_permissions(&path, permissions).map_err(|e| {
                        Diagnostic {
                            r#type: "error".to_string(),
                            message: format!("io.file.chmod failed: {}", e),
                            filename: node.filename.clone(),
                            line: node.line,
                            col: node.col,
                            slot: Some("io.file.chmod".to_string()),
                        }
                    })?;
                }
            }

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}
