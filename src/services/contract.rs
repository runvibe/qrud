use serde_json::Value;

#[derive(Clone)]
pub struct ApiContract {
    operations: Vec<OperationContract>,
}

#[derive(Clone)]
struct OperationContract {
    method: String,
    path_template: String,
    request_schema: Option<Value>,
}

impl ApiContract {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let spec: Value = serde_json::from_str(&content).map_err(|err| err.to_string())?;
        Self::from_spec(&spec)
    }

    fn from_spec(spec: &Value) -> Result<Self, String> {
        let mut operations = Vec::new();
        let paths = spec
            .get("paths")
            .and_then(Value::as_object)
            .ok_or_else(|| "OpenAPI spec must contain paths".to_string())?;

        for (path_template, path_item) in paths {
            let Some(path_obj) = path_item.as_object() else {
                continue;
            };
            for method in ["get", "post", "put", "patch", "delete"] {
                let Some(operation) = path_obj.get(method) else {
                    continue;
                };
                let request_schema = extract_request_schema(spec, operation)?;
                operations.push(OperationContract {
                    method: method.to_string(),
                    path_template: path_template.to_string(),
                    request_schema,
                });
            }
        }

        Ok(Self { operations })
    }

    pub fn validate_route(&self, method: &str, request_path: &str) -> bool {
        self.operations.iter().any(|op| {
            op.method.eq_ignore_ascii_case(method)
                && match_path_template(&op.path_template, request_path)
        })
    }

    pub fn validate_payload(
        &self,
        method: &str,
        request_path: &str,
        payload: &Value,
    ) -> Result<(), String> {
        let Some(op) = self.operations.iter().find(|op| {
            op.method.eq_ignore_ascii_case(method)
                && match_path_template(&op.path_template, request_path)
        }) else {
            return Err("Route not found in OpenAPI".to_string());
        };

        let Some(schema) = op.request_schema.as_ref() else {
            return Ok(());
        };

        let compiled = jsonschema::JSONSchema::compile(schema)
            .map_err(|err| format!("Invalid schema: {err}"))?;
        if let Err(mut errs) = compiled.validate(payload) {
            if let Some(first) = errs.next() {
                return Err(format!("Payload validation failed: {first}"));
            }
            return Err("Payload validation failed".to_string());
        }

        Ok(())
    }
}

fn extract_request_schema(spec: &Value, operation: &Value) -> Result<Option<Value>, String> {
    let request_body = operation.get("requestBody");
    let Some(request_body) = request_body else {
        return Ok(None);
    };
    let resolved_request_body = resolve_refs(spec, request_body, 0)?;

    let content = resolved_request_body
        .get("content")
        .and_then(Value::as_object)
        .ok_or_else(|| "requestBody missing content".to_string())?;

    let Some(media_type) = content.get("application/json") else {
        return Ok(None);
    };
    let Some(schema) = media_type.get("schema") else {
        return Ok(None);
    };
    let resolved_schema = resolve_refs(spec, schema, 0)?;
    Ok(Some(resolved_schema))
}

fn resolve_refs(spec: &Value, value: &Value, depth: u8) -> Result<Value, String> {
    if depth > 32 {
        return Err("OpenAPI ref resolution exceeded max depth".to_string());
    }
    match value {
        Value::Object(map) => {
            if let Some(ref_value) = map.get("$ref").and_then(Value::as_str) {
                if !ref_value.starts_with("#/") {
                    return Err("Only local OpenAPI refs are supported".to_string());
                }
                let pointer = &ref_value[1..];
                let referenced = spec
                    .pointer(pointer)
                    .ok_or_else(|| format!("OpenAPI ref not found: {ref_value}"))?;
                return resolve_refs(spec, referenced, depth + 1);
            }

            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_refs(spec, entry, depth + 1)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_refs(spec, item, depth + 1)?);
            }
            Ok(Value::Array(out))
        }
        _ => Ok(value.clone()),
    }
}

fn match_path_template(template: &str, request_path: &str) -> bool {
    let template = normalize_path(template);
    let request = normalize_path(request_path);

    let template_segments = split_segments(&template);
    let request_segments = split_segments(&request);
    if template_segments.len() != request_segments.len() {
        return false;
    }

    template_segments
        .iter()
        .zip(request_segments.iter())
        .all(|(template_seg, request_seg)| {
            if template_seg.starts_with('{') && template_seg.ends_with('}') {
                return !request_seg.is_empty();
            }
            template_seg == request_seg
        })
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn split_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|value| !value.is_empty())
        .collect()
}
