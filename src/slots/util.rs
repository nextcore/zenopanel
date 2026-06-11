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
}
