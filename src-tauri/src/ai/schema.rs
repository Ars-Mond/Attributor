//! Adapts the user's JSON Schema to the structured-output dialects of cloud providers, which accept only a subset of JSON Schema.
//! Local servers (Ollama, llama.cpp, LM Studio, Jan, vLLM, Unsloth) compile the schema into a grammar and receive it unchanged.

use serde_json::{Map, Value};

/// Keywords Claude's `output_config.format` rejects; the model still sees them in the schema appended to the prompt.
const ANTHROPIC_UNSUPPORTED: &[&str] = &[
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "uniqueItems",
    "contains",
    "minContains",
    "maxContains",
    "minProperties",
    "maxProperties",
    "patternProperties",
    "propertyNames",
];

/// Whether a schema value asks for structured output at all (the frontend sends `{}` when the configured schema is not valid JSON).
pub fn is_present(schema: &Value) -> bool {
    schema.as_object().is_some_and(|o| !o.is_empty())
}

/// OpenAI strict mode: every object must set `additionalProperties: false` and list all of its properties in `required`.
///
/// Returns the adapted schema and whether strict mode can be requested; a schema with optional properties or open objects is sent non-strict.
pub fn for_openai(schema: &Value) -> (Value, bool) {
    let mut out = schema.clone();
    let mut strictness = Strictness {
        closed: true,
        all_required: true,
        force: false,
    };
    close_objects(&mut out, &mut strictness);
    (out, strictness.closed && strictness.all_required)
}

/// Claude structured outputs: `additionalProperties: false` on every object (Claude accepts no other value and has no non-strict mode) and no numeric, length or array-size constraints.
pub fn for_anthropic(schema: &Value) -> Value {
    let mut out = schema.clone();
    strip_keywords(&mut out, ANTHROPIC_UNSUPPORTED);
    close_objects(
        &mut out,
        &mut Strictness {
            closed: true,
            all_required: true,
            force: true,
        },
    );
    out
}

/// What [`close_objects`] found out about whether a schema can run in OpenAI strict mode.
struct Strictness {
    /// No object explicitly allows additional properties.
    closed: bool,
    /// Every object lists all of its properties in `required`.
    all_required: bool,
    /// Overwrite an explicit `additionalProperties` with `false` instead of recording the object as open.
    force: bool,
}

/// Keys whose value maps property or definition names to schemas, so the map itself is not a schema node.
fn is_named_schema_map(key: &str) -> bool {
    matches!(key, "properties" | "$defs" | "definitions")
}

/// Keys whose value is instance data rather than a schema, and must not be rewritten.
fn is_instance_data(key: &str) -> bool {
    matches!(key, "enum" | "const" | "default" | "examples")
}

/// Visits every object schema (a node with `type: "object"` or `properties`) and sets `additionalProperties: false` where it is absent.
/// An object that explicitly allows additional properties is left as is and recorded in `strictness`, since closing it would change its meaning, unless `strictness.force` is set.
fn close_objects(node: &mut Value, strictness: &mut Strictness) {
    let obj = match node {
        Value::Object(obj) => obj,
        Value::Array(items) => {
            for value in items {
                close_objects(value, strictness);
            }
            return;
        }
        _ => return,
    };
    if obj.get("type").and_then(Value::as_str) == Some("object") || obj.contains_key("properties") {
        match obj.get("additionalProperties") {
            None => {
                obj.insert("additionalProperties".into(), Value::Bool(false));
            }
            Some(Value::Bool(false)) => {}
            Some(_) if strictness.force => {
                obj.insert("additionalProperties".into(), Value::Bool(false));
            }
            Some(_) => strictness.closed = false,
        }
        if !lists_all_properties(obj) {
            strictness.all_required = false;
        }
    }
    for (key, value) in obj.iter_mut() {
        if is_instance_data(key) {
            continue;
        }
        match value {
            Value::Object(named) if is_named_schema_map(key) => {
                for schema in named.values_mut() {
                    close_objects(schema, strictness);
                }
            }
            _ => close_objects(value, strictness),
        }
    }
}

/// Whether every key of the object's `properties` appears in its `required` array.
fn lists_all_properties(obj: &Map<String, Value>) -> bool {
    let required: Vec<&str> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    obj.get("properties")
        .and_then(Value::as_object)
        .is_none_or(|props| props.keys().all(|p| required.contains(&p.as_str())))
}

/// Removes `keywords` from every schema node. Keys of `properties` maps are property names, not keywords, and are kept.
fn strip_keywords(node: &mut Value, keywords: &[&str]) {
    let obj = match node {
        Value::Object(obj) => obj,
        Value::Array(items) => {
            for value in items {
                strip_keywords(value, keywords);
            }
            return;
        }
        _ => return,
    };
    obj.retain(|key, _| !keywords.contains(&key.as_str()));
    for (key, value) in obj.iter_mut() {
        if is_instance_data(key) {
            continue;
        }
        match value {
            Value::Object(named) if is_named_schema_map(key) => {
                for schema in named.values_mut() {
                    strip_keywords(schema, keywords);
                }
            }
            _ => strip_keywords(value, keywords),
        }
    }
}

/// Truncates top-level array properties of a parsed answer to the schema's `maxItems`, for providers that cannot enforce it.
pub fn clamp_arrays(answer: &mut Value, schema: &Value) {
    let (Some(answer), Some(props)) = (
        answer.as_object_mut(),
        schema.get("properties").and_then(Value::as_object),
    ) else {
        return;
    };
    for (name, prop) in props {
        let max = prop
            .get("maxItems")
            .and_then(Value::as_u64)
            .and_then(|m| usize::try_from(m).ok());
        if let (Some(max), Some(Value::Array(items))) = (max, answer.get_mut(name)) {
            items.truncate(max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "maxLength": 200},
                "keywords": {"type": "array", "items": {"type": "string"}, "maxItems": 50},
                "maxItems": {"type": "integer", "minimum": 0}
            },
            "required": ["title", "keywords", "maxItems"]
        })
    }

    #[test]
    fn openai_closes_objects_and_keeps_constraints() {
        let (schema, strict) = for_openai(&sample());
        assert!(strict);
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(schema["properties"]["keywords"]["maxItems"], json!(50));
    }

    #[test]
    fn openai_is_not_strict_with_optional_properties() {
        let mut s = sample();
        s["required"] = json!(["title"]);
        assert!(!for_openai(&s).1);
    }

    #[test]
    fn openai_is_not_strict_with_open_objects() {
        let mut s = sample();
        s["additionalProperties"] = json!(true);
        let (schema, strict) = for_openai(&s);
        assert!(!strict);
        assert_eq!(schema["additionalProperties"], json!(true));
    }

    #[test]
    fn anthropic_strips_constraints_but_not_property_names() {
        let schema = for_anthropic(&sample());
        assert_eq!(schema["additionalProperties"], json!(false));
        assert!(schema["properties"]["keywords"].get("maxItems").is_none());
        assert!(schema["properties"]["title"].get("maxLength").is_none());
        // A property literally named "maxItems" survives; only its own `minimum` keyword goes.
        assert_eq!(schema["properties"]["maxItems"], json!({"type": "integer"}));
    }

    #[test]
    fn anthropic_closes_explicitly_open_objects() {
        let mut s = sample();
        s["additionalProperties"] = json!(true);
        assert_eq!(for_anthropic(&s)["additionalProperties"], json!(false));
    }

    #[test]
    fn clamp_arrays_truncates_to_max_items() {
        let mut answer = json!({"keywords": ["a", "b", "c"], "title": "t"});
        let schema = json!({"properties": {"keywords": {"type": "array", "maxItems": 2}}});
        clamp_arrays(&mut answer, &schema);
        assert_eq!(answer["keywords"], json!(["a", "b"]));
    }

    #[test]
    fn empty_schema_is_not_present() {
        assert!(!is_present(&json!({})));
        assert!(!is_present(&Value::Null));
        assert!(is_present(&sample()));
    }
}
