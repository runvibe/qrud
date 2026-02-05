use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, Response};
use axum::middleware::Next;
use serde_json::map::Entry;
use serde_json::{Map, Value};
use std::net::SocketAddr;
use tracing::Level;

pub async fn log_request_response(req: Request<Body>, next: Next) -> Response<Body> {
    if !tracing::enabled!(Level::DEBUG) {
        return next.run(req).await;
    }

    let (parts, body) = req.into_parts();
    let origin = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let method = parts.method.to_string();
    let url = parts.uri.to_string();
    let headers = headers_to_json(&parts.headers);
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();

    let request_log = serde_json::json!({
        "origin": origin.clone(),
        "url": url.clone(),
        "headers": headers,
        "method": method.clone(),
        "body": body_text,
    });
    tracing::debug!(target: "http", "{}", request_log);

    let request = Request::from_parts(parts, Body::from(body_bytes));
    let response = next.run(request).await;

    let (parts, body) = response.into_parts();
    let headers = headers_to_json(&parts.headers);
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();
    let response_log = serde_json::json!({
        "origin": origin,
        "url": url,
        "headers": headers,
        "method": method,
        "body": body_text,
    });
    tracing::debug!(target: "http", "{}", response_log);

    Response::from_parts(parts, Body::from(body_bytes))
}

fn headers_to_json(headers: &HeaderMap) -> Value {
    let mut map = Map::new();
    for (name, value) in headers.iter() {
        let value = value
            .to_str()
            .map(str::to_string)
            .unwrap_or_else(|_| String::from_utf8_lossy(value.as_bytes()).to_string());
        match map.entry(name.as_str().to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(Value::String(value));
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Value::String(existing) => {
                    let existing = std::mem::take(existing);
                    entry.insert(Value::Array(vec![
                        Value::String(existing),
                        Value::String(value),
                    ]));
                }
                Value::Array(values) => values.push(Value::String(value)),
                other => {
                    let existing = std::mem::replace(other, Value::Null);
                    entry.insert(Value::Array(vec![existing, Value::String(value)]));
                }
            },
        }
    }
    Value::Object(map)
}
