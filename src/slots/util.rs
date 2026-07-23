use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use std::sync::Arc;
use std::collections::HashMap;
use super::{resolve_node_value, register_function, get_function};

pub fn register(engine: &mut Engine) {
    engine.register(
        "cast.to_int",
        Arc::new(|engine, _ctx, node, scope| {
            let mut val = Value::Nil;
            let mut target = "cast_result".to_string();

            if node.value.is_some() {
                val = resolve_node_value(engine, node, scope);
            }

            for child in &node.children {
                let child_val = engine.resolve_shorthand_value(child, scope);
                if child.name == "val" || child.name == "value" {
                    val = child_val;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let int_val = val.to_int();
            scope.set(&target, Value::Int(int_val));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "coalesce",
        Arc::new(|engine, _ctx, node, scope| {
            let mut val = Value::Nil;
            let mut def = Value::Nil;
            let mut target = "coalesce_result".to_string();

            if node.value.is_some() {
                val = resolve_node_value(engine, node, scope);
            }

            for child in &node.children {
                let child_val = engine.resolve_shorthand_value(child, scope);
                if child.name == "val" || child.name == "value" {
                    val = child_val;
                } else if child.name == "default" || child.name == "def" {
                    def = child_val;
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim().trim_start_matches('$').trim().to_string();
                }
            }

            let is_nil = match &val {
                Value::Nil => true,
                Value::String(s) => s.is_empty() || s == "nil" || s == "<nil>" || s.starts_with('$'),
                _ => false,
            };

            let result = if is_nil { def.clone() } else { val.clone() };
            scope.set(&target, result);
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "fn",
        Arc::new(|engine, _ctx, node, scope| {
            let func_name = resolve_node_value(engine, node, scope).to_string_coerce();
            if func_name.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "fn: function name is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("fn".to_string()),
                });
            }

            register_function(func_name, node.clone());
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "call",
        Arc::new(|engine, ctx, node, scope| {
            let func_name = resolve_node_value(engine, node, scope).to_string_coerce();
            if func_name.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "call: function name is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("call".to_string()),
                });
            }

            let func_node = get_function(&func_name).ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("call: function '{}' not found", func_name),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("call".to_string()),
                }
            })?;

            for child in &func_node.children {
                engine.execute(ctx, child, scope)?;
            }

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "include",
        Arc::new(|engine, ctx, node, scope| {
            let path = resolve_node_value(engine, node, scope).to_string_coerce();
            let content = std::fs::read_to_string(&path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("include failed to read file '{}': {}", path, e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("include".to_string()),
                }
            })?;
            
            let parsed_node = zenocore::parser::parse_string(&content, &path).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("include failed to parse file '{}': {:?}", path, e),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("include".to_string()),
                }
            })?;

            engine.execute(ctx, &parsed_node, scope)
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "if",
        Arc::new(|engine, ctx, node, scope| {
            let mut then_node = None;
            let mut else_node = None;

            let cond_val = if let Some(ref val_str) = node.value {
                evaluate_condition(engine, val_str, scope)
            } else {
                if let Some(cond_node) = node.children.first() {
                    for child in &cond_node.children {
                        if child.name == "then" {
                            then_node = Some(child);
                        } else if child.name == "else" {
                            else_node = Some(child);
                        }
                    }

                    match cond_node.name.as_str() {
                        "not_empty" => {
                            let val_str = cond_node.value.as_deref().unwrap_or("").trim();
                            let val = resolve_expression_value(engine, val_str, scope);
                            match val {
                                Value::Nil => false,
                                Value::String(s) => !s.is_empty() && s != "nil" && s != "<nil>" && !s.starts_with('$'),
                                Value::List(l) => !l.is_empty(),
                                Value::Map(m) => !m.is_empty(),
                                Value::Bool(b) => b,
                                Value::Int(i) => i != 0,
                                Value::Float(f) => f != 0.0,
                            }
                        }
                        "is_empty" => {
                            let val_str = cond_node.value.as_deref().unwrap_or("").trim();
                            let val = resolve_expression_value(engine, val_str, scope);
                            match val {
                                Value::Nil => true,
                                Value::String(s) => s.is_empty() || s == "nil" || s == "<nil>" || s.starts_with('$'),
                                Value::List(l) => l.is_empty(),
                                Value::Map(m) => m.is_empty(),
                                Value::Bool(b) => !b,
                                Value::Int(i) => i == 0,
                                Value::Float(f) => f == 0.0,
                            }
                        }
                        "contains" => {
                            let val_str = cond_node.value.as_deref().unwrap_or("").trim();
                            let parts: Vec<&str> = val_str.split(',').collect();
                            if parts.len() >= 2 {
                                let main_expr = parts[0].trim();
                                let main_val = resolve_expression_value(engine, main_expr, scope).to_string_coerce();
                                
                                let mut search_val = String::new();
                                let second_part = parts[1].trim();
                                if second_part.contains(':') {
                                    let sub_parts: Vec<&str> = second_part.splitn(2, ':').collect();
                                    if sub_parts.len() == 2 {
                                        search_val = resolve_expression_value(engine, sub_parts[1].trim(), scope).to_string_coerce();
                                    }
                                }
                                main_val.contains(&search_val)
                            } else {
                                false
                            }
                        }
                        _ => {
                            eprintln!("[IF WARNING] Unknown block condition: {}", cond_node.name);
                            false
                        }
                    }
                } else {
                    false
                }
            };

            if node.value.is_some() {
                for child in &node.children {
                    if child.name == "then" {
                        then_node = Some(child);
                    } else if child.name == "else" {
                        else_node = Some(child);
                    }
                }
            }

            if cond_val {
                if let Some(then_n) = then_node {
                    for child in &then_n.children {
                        engine.execute(ctx, child, scope)?;
                    }
                }
            } else if let Some(else_n) = else_node {
                for child in &else_n.children {
                    engine.execute(ctx, child, scope)?;
                }
            }

            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "log",
        Arc::new(|engine, _ctx, node, scope| {
            let val = if node.value.is_some() {
                resolve_node_value(engine, node, scope)
            } else {
                let mut map = HashMap::new();
                for child in &node.children {
                    let child_val = engine.resolve_shorthand_value(child, scope);
                    map.insert(child.name.clone(), child_val);
                }
                if map.is_empty() {
                    Value::Nil
                } else {
                    Value::Map(map)
                }
            };
            println!("[ZenoLang Log] {}", val.to_string_coerce());
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "util.datetime",
        Arc::new(|_engine, _ctx, node, scope| {
            let mut target = "datetime_result".to_string();
            for child in &node.children {
                if child.name == "as" {
                    if let Some(ref val) = child.value {
                        target = val.trim_start_matches('$').to_string();
                    }
                }
            }
            let now = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
            scope.set(&target, Value::String(now));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "util.is_download_url",
        Arc::new(|engine, _ctx, node, scope| {
            let mut val = String::new();
            let mut target = "is_download".to_string();

            if node.value.is_some() {
                val = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let child_val = engine.resolve_shorthand_value(child, scope);
                if child.name == "val" || child.name == "value" {
                    val = child_val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                }
            }

            let is_download = val.to_lowercase().starts_with("http://") || val.to_lowercase().starts_with("https://");
            scope.set(&target, Value::Bool(is_download));
            Ok(())
        }),
        SlotMeta { description: "Check if value starts with http:// or https://".to_string(), example: "util.is_download_url: $source_url { as: $is_download }".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "bool".to_string() }
    );

    engine.register(
        "util.sync_local_isos",
        Arc::new(|_engine, ctx, node, _scope| {
            let db_mgr = ctx.get::<crate::db::DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "util.sync_local_isos: DBManager not found".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("util.sync_local_isos".to_string()),
                }
            })?;

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool_opt = db_mgr.get_pool("default").await;
                    if let Some(crate::db::DbPool::Sqlite(pool)) = pool_opt {
                        let dir_path = "/var/lib/zeno-container/isos";
                        let _ = std::fs::create_dir_all(dir_path);

                        // Read all physical files
                        let mut physical_files = HashMap::new();
                        if let Ok(entries) = std::fs::read_dir(dir_path) {
                            for entry in entries.flatten() {
                                if let Ok(meta) = entry.metadata() {
                                    if meta.is_file() {
                                        let name = entry.file_name().to_string_lossy().to_string();
                                        let path = entry.path().to_string_lossy().to_string();
                                        physical_files.insert(path, (name, meta.len()));
                                    }
                                }
                            }
                        }

                        // Retrieve registered ISOs under /var/lib/zeno-container/isos from db
                        let db_isos: Vec<(i64, String, String)> = sqlx::query_as::<_, (i64, String, String)>(
                            "SELECT id, path, status FROM db_isos WHERE path LIKE '/var/lib/zeno-container/isos/%'"
                        )
                        .fetch_all(&pool)
                        .await
                        .unwrap_or_default();

                        // Clean up database records whose physical files are missing
                        for (id, path, status) in db_isos {
                            // If status is 'downloading', do not delete it even if file doesn't exist yet!
                            if status != "downloading" && !physical_files.contains_key(&path) {
                                let _ = sqlx::query("DELETE FROM db_isos WHERE id = ?")
                                    .bind(id)
                                    .execute(&pool)
                                    .await;
                            }
                        }

                        // Insert new physical files not yet in database
                        for (path, (name, size)) in physical_files {
                            // Check if already exists in db
                            let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM db_isos WHERE path = ?")
                                .bind(&path)
                                .fetch_one(&pool)
                                .await
                                .unwrap_or(0) > 0;

                            if !exists {
                                let _ = sqlx::query(
                                    "INSERT INTO db_isos (name, size_bytes, path, source_url, status) VALUES (?, ?, ?, 'Local Storage', 'ready')"
                                )
                                .bind(name)
                                .bind(size as i64)
                                .bind(path)
                                .execute(&pool)
                                .await;
                            }
                        }
                    }
                })
            });

            Ok(())
        }),
        SlotMeta { description: "Sync local ISO files to database".to_string(), example: "util.sync_local_isos".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

fn evaluate_condition(engine: &Engine, expr: &str, scope: &Arc<zenocore::Scope>) -> bool {
    let mut expr = expr.trim();
    if (expr.starts_with('"') && expr.ends_with('"')) || (expr.starts_with('\'') && expr.ends_with('\'')) {
        expr = expr[1..expr.len()-1].trim();
    }
    if expr.is_empty() {
        return false;
    }

    if expr.contains("||") {
        for part in expr.split("||") {
            if evaluate_condition(engine, part, scope) {
                return true;
            }
        }
        return false;
    }

    if expr.contains("&&") {
        for part in expr.split("&&") {
            if !evaluate_condition(engine, part, scope) {
                return false;
            }
        }
        return true;
    }

    let ops = ["==", "!=", ">=", "<=", ">", "<"];
    for op in &ops {
        if expr.contains(op) {
            let parts: Vec<&str> = expr.splitn(2, op).collect();
            if parts.len() == 2 {
                let left_str = parts[0].trim();
                let right_str = parts[1].trim();

                let left_val = resolve_expression_value(engine, left_str, scope);
                let right_val = resolve_expression_value(engine, right_str, scope);

                return match *op {
                    "==" => left_val.to_string_coerce() == right_val.to_string_coerce(),
                    "!=" => left_val.to_string_coerce() != right_val.to_string_coerce(),
                    ">" => left_val.to_float() > right_val.to_float(),
                    "<" => left_val.to_float() < right_val.to_float(),
                    ">=" => left_val.to_float() >= right_val.to_float(),
                    "<=" => left_val.to_float() <= right_val.to_float(),
                    _ => false,
                };
            }
        }
    }

    let resolved = resolve_expression_value(engine, expr, scope);
    resolved.to_bool()
}

fn resolve_expression_value(_engine: &Engine, s: &str, scope: &Arc<zenocore::Scope>) -> Value {
    let s = s.trim();
    if s.starts_with('$') {
        let key = &s[1..];
        if key.contains('.') {
            let parts: Vec<&str> = key.splitn(2, '.').collect();
            if let Some(parent) = scope.get(parts[0]) {
                if let Value::Map(ref m) = parent {
                    return m.get(parts[1]).cloned().unwrap_or(Value::Nil);
                }
            }
            return Value::Nil;
        }
        return scope.get(key).unwrap_or(Value::Nil);
    }
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len()-1].to_string());
    }
    if s == "true" { return Value::Bool(true); }
    if s == "false" { return Value::Bool(false); }
    if s == "null" || s == "nil" { return Value::Nil; }
    if let Ok(i) = s.parse::<i64>() { return Value::Int(i); }
    if let Ok(f) = s.parse::<f64>() { return Value::Float(f); }
    Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn run_code(code: &str, scope: &Arc<zenocore::Scope>) {
        let mut engine = zenoengine::new_engine();
        crate::slots::register_custom_slots(&mut engine);
        let mut ctx = zenocore::Context::new();
        let parsed = zenocore::parser::parse_string(code, "test.zl").unwrap();
        engine.execute(&mut ctx, &parsed, scope).unwrap();
    }

    #[test]
    fn test_custom_if_slots() {
        let scope = zenocore::Scope::new(None);
        scope.set("driver", Value::String("mysql".to_string()));
        scope.set("q_connections", Value::String("SHOW STATUS".to_string()));
        scope.set("entered_then", Value::Int(0));
        scope.set("entered_else", Value::Int(0));

        let code_contains = "
        if: { contains: \"$driver, value: 'ys'\" } {
            then: {
                var: $entered_then { val: 1 }
            }
            else: {
                var: $entered_else { val: 1 }
            }
        }
        ";
        run_code(code_contains, &scope);
        assert_eq!(scope.get("entered_then").unwrap().to_int(), 1);
        assert_eq!(scope.get("entered_else").unwrap().to_int(), 0);

        // Test 4: contains evaluates to false (mysql does not contain postgres)
        scope.set("driver", Value::String("mysql:5.6".to_string()));
        scope.set("entered_then", Value::Int(0));
        scope.set("entered_else", Value::Int(0));
        let code_contains_false = "
        if: { contains: \"$driver, value: 'postgres'\" } {
            then: {
                var: $entered_then { val: 1 }
            }
            else: {
                var: $entered_else { val: 1 }
            }
        }
        ";
        run_code(code_contains_false, &scope);
        assert_eq!(scope.get("entered_then").unwrap().to_int(), 0);
        assert_eq!(scope.get("entered_else").unwrap().to_int(), 1);

        // Test 5: string comparison evaluates to true
        scope.set("driver", Value::String("mysql".to_string()));
        scope.set("entered_then", Value::Int(0));
        scope.set("entered_else", Value::Int(0));
        let code_str = "
        if: \"$driver == 'mysql'\" {
            then: {
                var: $entered_then { val: 1 }
            }
            else: {
                var: $entered_else { val: 1 }
            }
        }
        ";
        run_code(code_str, &scope);
        assert_eq!(scope.get("entered_then").unwrap().to_int(), 1);
        assert_eq!(scope.get("entered_else").unwrap().to_int(), 0);
    }

    #[test]
    fn test_coalesce_and_var() {
        let mut engine = zenoengine::new_engine();
        crate::slots::register_custom_slots(&mut engine);
        let mut ctx = zenocore::Context::new();
        let scope = zenocore::Scope::new(None);
        
        scope.set("driver", Value::String("mysql".to_string()));
        
        let code = "
        var: $connections { val: 0 }
        if: \"$driver == 'mysql'\" {
            then: {
                coalesce: $res_conn.Value {
                    default: '0'
                    as: $connections_str
                }
                var: $connections { val: $connections_str }
            }
        }
        ";
        let parsed = zenocore::parser::parse_string(code, "test.zl").unwrap();
        println!("COALESCE TEST AST: {:#?}", parsed);
        engine.execute(&mut ctx, &parsed, &scope).unwrap();
        
        let connections = scope.get("connections").unwrap().to_string_coerce();
        println!("connections: {}", connections);
        assert_eq!(connections, "0");
    }
}



