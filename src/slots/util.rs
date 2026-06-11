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
                    target = child.value.clone().unwrap_or_default().trim_start_matches('$').to_string();
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
            let cond_val = if let Some(ref val_str) = node.value {
                evaluate_condition(engine, val_str, scope)
            } else {
                false
            };

            let mut then_node = None;
            let mut else_node = None;

            for child in &node.children {
                if child.name == "then" {
                    then_node = Some(child);
                } else if child.name == "else" {
                    else_node = Some(child);
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
}

fn evaluate_condition(engine: &Engine, expr: &str, scope: &Arc<zenocore::Scope>) -> bool {
    let expr = expr.trim();
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
