use base64::Engine;
use fancy_regex::Regex;
use serde_json::Value;
use std::path::Path;
use url::Url;

#[derive(Clone)]
pub struct ApiContract {
    spec: Value,
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
        Self::from_source(path)
    }

    pub fn from_source(source: &str) -> Result<Self, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err("schema source cannot be empty".to_string());
        }

        if looks_like_inline_schema(source) {
            return Self::from_schema_text("inline", source).or_else(|inline_err| {
                let decoded = decode_base64_utf8(source).map_err(|decode_err| {
                    format!(
                        "failed to load schema source from inline content: inline parse failed: {inline_err}; base64 decode failed: {decode_err}"
                    )
                })?;
                Self::from_schema_text("inline.base64", &decoded).map_err(|base64_err| {
                    format!(
                        "failed to load schema source from inline content: inline parse failed: {inline_err}; base64 parse failed: {base64_err}"
                    )
                })
            });
        }

        if let Some(url) = parse_http_url(source) {
            return Self::from_remote_url(&url);
        }

        match std::fs::read_to_string(source) {
            Ok(content) => Self::from_schema_text(source, &content),
            Err(file_err) => {
                let inline_result = Self::from_schema_text("inline", source);
                if let Ok(contract) = inline_result {
                    return Ok(contract);
                }
                let inline_err = inline_result.err().expect("inline result checked");

                let decoded = decode_base64_utf8(source).map_err(|decode_err| {
                    format!(
                        "failed to load schema source: file read failed for `{source}`: {file_err}; inline parse failed: {inline_err}; base64 decode failed: {decode_err}"
                    )
                })?;

                Self::from_schema_text("inline.base64", &decoded).map_err(|base64_err| {
                    format!(
                        "failed to load schema source: file read failed for `{source}`: {file_err}; inline parse failed: {inline_err}; base64 parse failed: {base64_err}"
                    )
                })
            }
        }
    }

    fn from_remote_url(url: &Url) -> Result<Self, String> {
        let response = reqwest::blocking::get(url.as_str())
            .map_err(|err| format!("failed to fetch schema from `{url}`: {err}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "failed to fetch schema from `{url}`: HTTP {}",
                status
            ));
        }

        let content = response
            .text()
            .map_err(|err| format!("failed to read schema body from `{url}`: {err}"))?;
        let source_name = if url.path().is_empty() || url.path() == "/" {
            url.as_str()
        } else {
            url.path()
        };
        Self::from_schema_text(source_name, &content)
    }

    fn from_schema_text(source_name: &str, content: &str) -> Result<Self, String> {
        let spec = parse_openapi_spec(source_name, content)?;
        Self::from_spec(&spec)
    }

    pub fn from_value(spec: &Value) -> Result<Self, String> {
        Self::from_spec(spec)
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

        Ok(Self {
            spec: spec.clone(),
            operations,
        })
    }

    pub fn openapi_spec(&self) -> &Value {
        &self.spec
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

        let compiled = jsonschema::options()
            .should_validate_formats(true)
            .build(schema)
            .map_err(|err| format!("Invalid schema: {err}"))?;
        if let Err(err) = compiled.validate(payload) {
            return Err(format!("Payload validation failed: {err}"));
        }
        if let Err(err) = validate_schema_patterns(schema, payload) {
            return Err(format!("Payload validation failed: {err}"));
        }

        Ok(())
    }
}

fn parse_http_url(source: &str) -> Option<Url> {
    let url = Url::parse(source).ok()?;
    match url.scheme() {
        "http" | "https" => Some(url),
        _ => None,
    }
}

fn looks_like_inline_schema(source: &str) -> bool {
    matches!(
        source.chars().find(|ch| !ch.is_whitespace()),
        Some('{') | Some('[')
    )
}

fn decode_base64_utf8(source: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(source)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(source))
        .map_err(|err| err.to_string())?;

    String::from_utf8(bytes).map_err(|err| err.to_string())
}

fn parse_openapi_spec(path: &str, content: &str) -> Result<Value, String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("json") => {
            serde_json::from_str(content).map_err(|err| format!("invalid JSON schema file: {err}"))
        }
        Some("yaml") | Some("yml") => parse_yaml_to_json(content),
        _ => match serde_json::from_str(content) {
            Ok(spec) => Ok(spec),
            Err(json_err) => match parse_yaml_to_json(content) {
                Ok(spec) => Ok(spec),
                Err(yaml_err) => Err(format!(
                    "invalid schema file; JSON error: {json_err}; YAML error: {yaml_err}"
                )),
            },
        },
    }
}

fn parse_yaml_to_json(content: &str) -> Result<Value, String> {
    let yaml_value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|err| format!("invalid YAML schema file: {err}"))?;
    serde_json::to_value(yaml_value).map_err(|err| format!("invalid YAML schema file: {err}"))
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
    let mut resolved_schema = resolve_refs(spec, schema, 0)?;
    apply_openapi_defaults(&mut resolved_schema);
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

fn apply_openapi_defaults(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            let has_properties = map.get("properties").and_then(Value::as_object).is_some();
            let has_required = map.get("required").and_then(Value::as_array).is_some();
            if (has_properties || has_required) && !map.contains_key("additionalProperties") {
                map.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            for entry in map.values_mut() {
                apply_openapi_defaults(entry);
            }
        }
        Value::Array(items) => {
            for item in items {
                apply_openapi_defaults(item);
            }
        }
        _ => {}
    }
}

fn validate_schema_patterns(schema: &Value, payload: &Value) -> Result<(), String> {
    validate_schema_patterns_at(schema, payload, "")
}

fn validate_schema_patterns_at(schema: &Value, payload: &Value, path: &str) -> Result<(), String> {
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        if let Some(value) = payload.as_str() {
            let regex =
                Regex::new(pattern).map_err(|err| format!("Invalid pattern at {path}: {err}"))?;
            let matches = regex
                .is_match(value)
                .map_err(|err| format!("Invalid pattern at {path}: {err}"))?;
            if !matches {
                return Err(format!(
                    "Property {} does not match pattern",
                    if path.is_empty() { "<root>" } else { path }
                ));
            }
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for entry in all_of {
            validate_schema_patterns_at(entry, payload, path)?;
        }
    }

    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        if !any_of.is_empty() {
            let mut matched = false;
            let mut last_err = None;
            for entry in any_of {
                match validate_schema_patterns_at(entry, payload, path) {
                    Ok(()) => {
                        matched = true;
                        break;
                    }
                    Err(err) => last_err = Some(err),
                }
            }
            if !matched {
                if let Some(err) = last_err {
                    return Err(err);
                }
            }
        }
    }

    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        if !one_of.is_empty() {
            let mut matches = 0;
            let mut last_err = None;
            for entry in one_of {
                match validate_schema_patterns_at(entry, payload, path) {
                    Ok(()) => matches += 1,
                    Err(err) => last_err = Some(err),
                }
            }
            if matches == 0 {
                if let Some(err) = last_err {
                    return Err(err);
                }
            }
        }
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        if let Some(object) = payload.as_object() {
            for (name, prop_schema) in properties {
                if let Some(value) = object.get(name) {
                    let next_path = if path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{path}.{name}")
                    };
                    validate_schema_patterns_at(prop_schema, value, &next_path)?;
                }
            }
        }
    }

    if let Some(items_schema) = schema.get("items") {
        if let Some(items) = payload.as_array() {
            match items_schema {
                Value::Array(schemas) => {
                    for (index, item) in items.iter().enumerate() {
                        let schema_for_item = schemas.get(index).or_else(|| schemas.last());
                        if let Some(schema_for_item) = schema_for_item {
                            let next_path = format_array_path(path, index);
                            validate_schema_patterns_at(schema_for_item, item, &next_path)?;
                        }
                    }
                }
                _ => {
                    for (index, item) in items.iter().enumerate() {
                        let next_path = format_array_path(path, index);
                        validate_schema_patterns_at(items_schema, item, &next_path)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn format_array_path(path: &str, index: usize) -> String {
    if path.is_empty() {
        format!("[{index}]")
    } else {
        format!("{path}[{index}]")
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
