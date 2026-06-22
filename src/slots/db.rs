use zenocore::{Engine, SlotMeta, Value, Diagnostic};
use crate::db::DBManager;
use super::resolve_node_value;
use std::sync::Arc;
use std::collections::HashMap;

pub fn register(engine: &mut Engine) {
    engine.register(
        "db.select",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.select: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.select".to_string()),
                }
            })?;

            let mut db_name = "default".to_string();
            let mut query_sql = String::new();
            let mut bind_args = Vec::new();
            let mut target = "rows".to_string();
            let mut only_first = false;

            if node.value.is_some() {
                query_sql = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "sql" {
                    query_sql = val.to_string_coerce();
                } else if child.name == "db" || child.name == "connection" {
                    db_name = val.to_string_coerce();
                } else if child.name == "as" {
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
                } else if child.name == "first" {
                    only_first = val.to_bool();
                } else if child.name == "bind" {
                    for bind_child in &child.children {
                        let bind_val = engine.resolve_shorthand_value(bind_child, scope);
                        bind_args.push(bind_val);
                    }
                }
            }

            let results = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool_opt = db_mgr.get_pool(&db_name).await;
                    match pool_opt {
                        Some(crate::db::DbPool::Sqlite(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let rows = query.fetch_all(&pool).await.map_err(|e| e.to_string())?;
                            let mut res_list = Vec::new();
                            use sqlx::{Row, Column, TypeInfo};
                            for row in rows {
                                let mut map = HashMap::new();
                                for col in row.columns() {
                                    let col_name = col.name().to_string();
                                    let val = match col.type_info().name() {
                                        "INTEGER" | "INT" | "BIGINT" => {
                                            if let Ok(v) = row.try_get::<i64, _>(col.ordinal()) {
                                                Value::Int(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "REAL" | "DOUBLE" | "FLOAT" => {
                                            if let Ok(v) = row.try_get::<f64, _>(col.ordinal()) {
                                                Value::Float(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "BOOLEAN" | "BOOL" => {
                                            if let Ok(v) = row.try_get::<bool, _>(col.ordinal()) {
                                                Value::Bool(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        _ => {
                                            if let Ok(v) = row.try_get::<String, _>(col.ordinal()) {
                                                Value::String(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                    };
                                    map.insert(col_name, val);
                                }
                                res_list.push(Value::Map(map));
                            }
                            Ok::<_, String>(res_list)
                        }
                        Some(crate::db::DbPool::MySql(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let rows = query.fetch_all(&pool).await.map_err(|e| e.to_string())?;
                            let mut res_list = Vec::new();
                            use sqlx::{Row, Column, TypeInfo};
                            for row in rows {
                                let mut map = HashMap::new();
                                for col in row.columns() {
                                    let col_name = col.name().to_string();
                                    let val = match col.type_info().name() {
                                        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" => {
                                            if let Ok(v) = row.try_get::<i64, _>(col.ordinal()) {
                                                Value::Int(v)
                                            } else if let Ok(v) = row.try_get::<i32, _>(col.ordinal()) {
                                                Value::Int(v as i64)
                                            } else if let Ok(v) = row.try_get::<i16, _>(col.ordinal()) {
                                                Value::Int(v as i64)
                                            } else if let Ok(v) = row.try_get::<i8, _>(col.ordinal()) {
                                                Value::Int(v as i64)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "DECIMAL" | "FLOAT" | "DOUBLE" => {
                                            if let Ok(v) = row.try_get::<f64, _>(col.ordinal()) {
                                                Value::Float(v)
                                            } else if let Ok(v) = row.try_get::<f32, _>(col.ordinal()) {
                                                Value::Float(v as f64)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "BIT" | "BOOLEAN" | "BOOL" => {
                                            if let Ok(v) = row.try_get::<bool, _>(col.ordinal()) {
                                                Value::Bool(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        _ => {
                                            if let Ok(v) = row.try_get::<String, _>(col.ordinal()) {
                                                Value::String(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                    };
                                    map.insert(col_name, val);
                                }
                                res_list.push(Value::Map(map));
                            }
                            Ok::<_, String>(res_list)
                        }
                        Some(crate::db::DbPool::Postgres(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let rows = query.fetch_all(&pool).await.map_err(|e| e.to_string())?;
                            let mut res_list = Vec::new();
                            use sqlx::{Row, Column, TypeInfo};
                            for row in rows {
                                let mut map = HashMap::new();
                                for col in row.columns() {
                                    let col_name = col.name().to_string();
                                    let val = match col.type_info().name() {
                                        "INT2" | "SMALLINT" | "INT4" | "INT" | "INTEGER" | "INT8" | "BIGINT" => {
                                            if let Ok(v) = row.try_get::<i64, _>(col.ordinal()) {
                                                Value::Int(v)
                                            } else if let Ok(v) = row.try_get::<i32, _>(col.ordinal()) {
                                                Value::Int(v as i64)
                                            } else if let Ok(v) = row.try_get::<i16, _>(col.ordinal()) {
                                                Value::Int(v as i64)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "FLOAT4" | "REAL" | "FLOAT8" | "DOUBLE PRECISION" | "NUMERIC" => {
                                            if let Ok(v) = row.try_get::<f64, _>(col.ordinal()) {
                                                Value::Float(v)
                                            } else if let Ok(v) = row.try_get::<f32, _>(col.ordinal()) {
                                                Value::Float(v as f64)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        "BOOL" | "BOOLEAN" => {
                                            if let Ok(v) = row.try_get::<bool, _>(col.ordinal()) {
                                                Value::Bool(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                        _ => {
                                            if let Ok(v) = row.try_get::<String, _>(col.ordinal()) {
                                                Value::String(v)
                                            } else {
                                                Value::Nil
                                            }
                                        }
                                    };
                                    map.insert(col_name, val);
                                }
                                res_list.push(Value::Map(map));
                            }
                            Ok::<_, String>(res_list)
                        }
                        None => {
                            Err(format!("database connection '{}' not found", db_name))
                        }
                    }
                })
            }).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: e,
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.select".to_string()),
                }
            })?;

            if only_first {
                let first_val = results.into_iter().next().unwrap_or(Value::Nil);
                scope.set(&target, first_val);
            } else {
                scope.set(&target, Value::List(results));
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.execute",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.execute: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.execute".to_string()),
                }
            })?;

            let mut db_name = "default".to_string();
            let mut query_sql = String::new();
            let mut bind_args = Vec::new();

            if node.value.is_some() {
                query_sql = resolve_node_value(engine, node, scope).to_string_coerce();
            }

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                if child.name == "sql" {
                    query_sql = val.to_string_coerce();
                } else if child.name == "db" || child.name == "connection" {
                    db_name = val.to_string_coerce();
                } else if child.name == "bind" {
                    for bind_child in &child.children {
                        let bind_val = engine.resolve_shorthand_value(bind_child, scope);
                        bind_args.push(bind_val);
                    }
                }
            }

            let (affected, last_id) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool_opt = db_mgr.get_pool(&db_name).await;
                    match pool_opt {
                        Some(crate::db::DbPool::Sqlite(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let res = query.execute(&pool).await.map_err(|e| e.to_string())?;
                            Ok::<_, String>((res.rows_affected() as i64, res.last_insert_rowid()))
                        }
                        Some(crate::db::DbPool::MySql(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let res = query.execute(&pool).await.map_err(|e| e.to_string())?;
                            Ok::<_, String>((res.rows_affected() as i64, res.last_insert_id() as i64))
                        }
                        Some(crate::db::DbPool::Postgres(pool)) => {
                            let mut query = sqlx::query(&query_sql);
                            for arg in bind_args {
                                query = match arg {
                                    Value::Nil => query.bind(None::<String>),
                                    Value::String(s) => query.bind(s),
                                    Value::Int(i) => query.bind(i),
                                    Value::Float(f) => query.bind(f),
                                    Value::Bool(b) => query.bind(b),
                                    _ => query.bind(arg.to_string_coerce()),
                                };
                            }
                            
                            let res = query.execute(&pool).await.map_err(|e| e.to_string())?;
                            Ok::<_, String>((res.rows_affected() as i64, 0))
                        }
                        None => {
                            Err(format!("database connection '{}' not found", db_name))
                        }
                    }
                })
            }).map_err(|e| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: e,
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.execute".to_string()),
                }
            })?;

            scope.set("db_affected", Value::Int(affected));
            scope.set("db_last_id", Value::Int(last_id));
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.connect",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.connect: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.connect".to_string()),
                }
            })?;

            let mut name = String::new();
            let mut driver = String::new();
            let mut host = String::new();
            let mut port = 0;
            let mut user = String::new();
            let mut password = String::new();
            let mut database = String::new();
            let mut target = "connect_result".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "name" => name = val.to_string_coerce(),
                    "driver" => driver = val.to_string_coerce(),
                    "host" => host = val.to_string_coerce(),
                    "port" => port = val.to_int() as u16,
                    "user" => user = val.to_string_coerce(),
                    "password" => password = val.to_string_coerce(),
                    "database" => database = val.to_string_coerce(),
                    "as" => {
                        if let Some(ref v) = child.value {
                            target = v.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            let res = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if driver == "mysql" {
                        db_mgr.add_mysql_connection(&name, &host, port, &user, &password, &database).await
                    } else if driver == "postgres" {
                        db_mgr.add_postgres_connection(&name, &host, port, &user, &password, &database).await
                    } else if driver == "sqlite" {
                        db_mgr.add_sqlite_connection(&name, &host).await
                    } else {
                        Err(sqlx::Error::Configuration("Unsupported driver".into()))
                    }
                })
            });

            match res {
                Ok(_) => {
                    scope.set(&target, Value::Bool(true));
                }
                Err(e) => {
                    scope.set(&target, Value::String(e.to_string()));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.disconnect",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.disconnect: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.disconnect".to_string()),
                }
            })?;

            let mut name = String::new();
            if node.value.is_some() {
                name = resolve_node_value(engine, node, scope).to_string_coerce();
            }
            for child in &node.children {
                if child.name == "name" {
                    name = engine.resolve_shorthand_value(child, scope).to_string_coerce();
                }
            }

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    db_mgr.pools.write().await.remove(&name);
                })
            });
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.backup",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.backup: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.backup".to_string()),
                }
            })?;

            let mut driver = String::new();
            let mut host = String::new();
            let mut port = 0;
            let mut user = String::new();
            let mut password = String::new();
            let mut database = String::new();
            let mut backup_path = String::new();
            let mut target = "backup_result".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "driver" => driver = val.to_string_coerce(),
                    "host" => host = val.to_string_coerce(),
                    "port" => port = val.to_int() as u16,
                    "user" => user = val.to_string_coerce(),
                    "password" => password = val.to_string_coerce(),
                    "database" => database = val.to_string_coerce(),
                    "backup_path" => backup_path = val.to_string_coerce(),
                    "as" => {
                        if let Some(ref v) = child.value {
                            target = v.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if backup_path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.backup: backup_path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.backup".to_string()),
                });
            }

            // Create parent directories if they don't exist
            if let Some(parent) = std::path::Path::new(&backup_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if driver == "sqlite" {
                        // Vacuum SQLite
                        let pool_name = if database.is_empty() || database == "default" {
                            "default".to_string()
                        } else {
                            database.clone()
                        };
                        let pool_opt = db_mgr.get_pool(&pool_name).await;
                        match pool_opt {
                            Some(crate::db::DbPool::Sqlite(pool)) => {
                                sqlx::query(&format!("VACUUM INTO '{}'", backup_path.replace('\'', "''")))
                                    .execute(&pool)
                                    .await
                                    .map(|_| ())
                                    .map_err(|e| e.to_string())
                            }
                            _ => Err(format!("SQLite pool '{}' not found", pool_name)),
                        }
                    } else if driver == "mysql" {
                        // Execute mysqldump
                        let file = std::fs::File::create(&backup_path)
                            .map_err(|e| format!("Failed to create backup file: {}", e))?;
                        let mut cmd = std::process::Command::new("mysqldump");
                        cmd.arg("-h").arg(&host)
                           .arg("-P").arg(&port.to_string())
                           .arg("-u").arg(&user);
                        if !password.is_empty() {
                            cmd.arg(format!("-p{}", password));
                        }
                        let output = cmd.arg(&database)
                           .stdout(std::process::Stdio::from(file))
                           .output();
                        match output {
                            Ok(out) => {
                                if out.status.success() {
                                    Ok(())
                                } else {
                                    let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                                    let _ = std::fs::remove_file(&backup_path);
                                    Err(format!("mysqldump failed: {}", err_msg))
                                }
                            }
                            Err(e) => {
                                let _ = std::fs::remove_file(&backup_path);
                                Err(format!("Failed to start mysqldump: {}", e))
                            }
                        }
                    } else if driver == "postgres" {
                        // Execute pg_dump
                        let file = std::fs::File::create(&backup_path)
                            .map_err(|e| format!("Failed to create backup file: {}", e))?;
                        let mut cmd = std::process::Command::new("pg_dump");
                        if !password.is_empty() {
                            cmd.env("PGPASSWORD", &password);
                        }
                        cmd.arg("-h").arg(&host)
                           .arg("-p").arg(&port.to_string())
                           .arg("-U").arg(&user)
                           .arg(&database)
                           .stdout(std::process::Stdio::from(file));
                        let output = cmd.output();
                        match output {
                            Ok(out) => {
                                if out.status.success() {
                                    Ok(())
                                } else {
                                    let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                                    let _ = std::fs::remove_file(&backup_path);
                                    Err(format!("pg_dump failed: {}", err_msg))
                                }
                            }
                            Err(e) => {
                                let _ = std::fs::remove_file(&backup_path);
                                Err(format!("Failed to start pg_dump: {}", e))
                            }
                        }
                    } else {
                        Err(format!("Unsupported database driver: {}", driver))
                    }
                })
            });

            match result {
                Ok(_) => {
                    let size = std::fs::metadata(&backup_path)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    scope.set(&target, Value::Map({
                        let mut m = HashMap::new();
                        m.insert("success".to_string(), Value::Bool(true));
                        m.insert("size".to_string(), Value::Int(size as i64));
                        m
                    }));
                }
                Err(e) => {
                    scope.set(&target, Value::Map({
                        let mut m = HashMap::new();
                        m.insert("success".to_string(), Value::Bool(false));
                        m.insert("error".to_string(), Value::String(e));
                        m
                    }));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );

    engine.register(
        "db.restore",
        Arc::new(|engine, ctx, node, scope| {
            let db_mgr = ctx.get::<DBManager>("db_manager").ok_or_else(|| {
                Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.restore: DBManager not found in context".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.restore".to_string()),
                }
            })?;

            let mut driver = String::new();
            let mut host = String::new();
            let mut port = 0;
            let mut user = String::new();
            let mut password = String::new();
            let mut database = String::new();
            let mut backup_path = String::new();
            let mut target = "restore_result".to_string();

            for child in &node.children {
                let val = engine.resolve_shorthand_value(child, scope);
                match child.name.as_str() {
                    "driver" => driver = val.to_string_coerce(),
                    "host" => host = val.to_string_coerce(),
                    "port" => port = val.to_int() as u16,
                    "user" => user = val.to_string_coerce(),
                    "password" => password = val.to_string_coerce(),
                    "database" => database = val.to_string_coerce(),
                    "backup_path" => backup_path = val.to_string_coerce(),
                    "as" => {
                        if let Some(ref v) = child.value {
                            target = v.trim_start_matches('$').to_string();
                        }
                    }
                    _ => {}
                }
            }

            if backup_path.is_empty() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: "db.restore: backup_path is required".to_string(),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.restore".to_string()),
                });
            }

            if !std::path::Path::new(&backup_path).exists() {
                return Err(Diagnostic {
                    r#type: "error".to_string(),
                    message: format!("db.restore: backup file not found at {}", backup_path),
                    filename: node.filename.clone(),
                    line: node.line,
                    col: node.col,
                    slot: Some("db.restore".to_string()),
                });
            }

            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    if driver == "sqlite" {
                        let pool_name = if database.is_empty() || database == "default" {
                            "default".to_string()
                        } else {
                            database.clone()
                        };

                        let db_file_path = if pool_name == "default" {
                            std::env::var("DB_NAME").unwrap_or_else(|_| "./zeno.db".to_string())
                        } else {
                            pool_name.clone()
                        };

                        // Disconnect/remove active pool to avoid locks
                        db_mgr.pools.write().await.remove(&pool_name);

                        // Copy backup file over active file
                        let copy_res = std::fs::copy(&backup_path, &db_file_path)
                            .map(|_| ())
                            .map_err(|e| format!("Failed to overwrite SQLite file: {}", e));

                        // Re-open/reconnect connection pool
                        if let Err(e) = db_mgr.add_sqlite_connection(&pool_name, &db_file_path).await {
                            eprintln!("Failed to re-initialize sqlite pool '{}': {}", pool_name, e);
                        }

                        copy_res
                    } else if driver == "mysql" {
                        // Execute mysql import
                        let file = std::fs::File::open(&backup_path)
                            .map_err(|e| format!("Failed to open backup file: {}", e))?;
                        let mut cmd = std::process::Command::new("mysql");
                        cmd.arg("-h").arg(&host)
                           .arg("-P").arg(&port.to_string())
                           .arg("-u").arg(&user);
                        if !password.is_empty() {
                            cmd.arg(format!("-p{}", password));
                        }
                        let output = cmd.arg(&database)
                           .stdin(std::process::Stdio::from(file))
                           .output();
                        match output {
                            Ok(out) => {
                                if out.status.success() {
                                    Ok(())
                                } else {
                                    let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                                    Err(format!("mysql restore failed: {}", err_msg))
                                }
                            }
                            Err(e) => Err(format!("Failed to start mysql client: {}", e)),
                        }
                    } else if driver == "postgres" {
                        // Execute psql import
                        let file = std::fs::File::open(&backup_path)
                            .map_err(|e| format!("Failed to open backup file: {}", e))?;
                        let mut cmd = std::process::Command::new("psql");
                        if !password.is_empty() {
                            cmd.env("PGPASSWORD", &password);
                        }
                        cmd.arg("-h").arg(&host)
                           .arg("-p").arg(&port.to_string())
                           .arg("-U").arg(&user)
                           .arg(&database)
                           .stdin(std::process::Stdio::from(file));
                        let output = cmd.output();
                        match output {
                            Ok(out) => {
                                if out.status.success() {
                                    Ok(())
                                } else {
                                    let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                                    Err(format!("psql restore failed: {}", err_msg))
                                }
                            }
                            Err(e) => Err(format!("Failed to start psql client: {}", e)),
                        }
                    } else {
                        Err(format!("Unsupported database driver: {}", driver))
                    }
                })
            });

            match result {
                Ok(_) => {
                    scope.set(&target, Value::Map({
                        let mut m = HashMap::new();
                        m.insert("success".to_string(), Value::Bool(true));
                        m
                    }));
                }
                Err(e) => {
                    scope.set(&target, Value::Map({
                        let mut m = HashMap::new();
                        m.insert("success".to_string(), Value::Bool(false));
                        m.insert("error".to_string(), Value::String(e));
                        m
                    }));
                }
            }
            Ok(())
        }),
        SlotMeta { description: "".to_string(), example: "".to_string(), inputs: HashMap::new(), required_blocks: Vec::new(), value_type: "".to_string() }
    );
}

