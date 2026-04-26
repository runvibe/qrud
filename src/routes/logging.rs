use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, Response, StatusCode};
use axum::middleware::Next;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use serde_json::map::Entry;
use serde_json::{Map, Value};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Instant;
use tracing::Level;

pub async fn log_request_response(req: Request<Body>, next: Next) -> Response<Body> {
    let debug_enabled = tracing::enabled!(target: "http", Level::DEBUG);
    let started_at = Instant::now();
    let (parts, body) = req.into_parts();
    let origin = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let method = parts.method.to_string();
    let url = parts.uri.to_string();

    let request = if debug_enabled {
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
        Request::from_parts(parts, Body::from(body_bytes))
    } else {
        Request::from_parts(parts, body)
    };

    let response = next.run(request).await;
    let (parts, body) = response.into_parts();
    let status = parts.status;
    let duration = started_at.elapsed();
    record_http_metrics(&method, &url, status, duration.as_secs_f64());

    if debug_enabled || status.is_client_error() || status.is_server_error() {
        let headers = headers_to_json(&parts.headers);
        let body_bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
        let body_text = String::from_utf8_lossy(&body_bytes).to_string();
        let response_log = serde_json::json!({
            "origin": origin,
            "url": url,
            "headers": headers,
            "method": method,
            "status": status.as_u16(),
            "duration_ms": duration.as_millis(),
            "body": body_text,
        });
        if debug_enabled {
            tracing::debug!(target: "http", "{}", response_log);
        }
        log_http_summary(
            &method,
            &url,
            status,
            duration.as_millis(),
            extract_error(&body_text),
        );
        return Response::from_parts(parts, Body::from(body_bytes));
    }

    log_http_summary(&method, &url, status, duration.as_millis(), None);
    Response::from_parts(parts, body)
}

fn record_http_metrics(method: &str, url: &str, status: StatusCode, duration_seconds: f64) {
    let attributes = [
        KeyValue::new("http.request.method", method.to_string()),
        KeyValue::new("http.response.status_code", status.as_u16() as i64),
        KeyValue::new("http.route", normalize_route(url)),
    ];
    http_request_duration().record(duration_seconds, &attributes);
    if status.is_client_error() || status.is_server_error() {
        http_errors_total().add(1, &attributes);
    }
}

fn log_http_summary(
    method: &str,
    url: &str,
    status: StatusCode,
    duration_ms: u128,
    error: Option<String>,
) {
    let status_code = status.as_u16();
    if status.is_server_error() {
        tracing::error!(
            target: "http",
            method = %method,
            url = %url,
            status = status_code,
            duration_ms = duration_ms,
            error = error.as_deref().unwrap_or(""),
            "http request completed"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            target: "http",
            method = %method,
            url = %url,
            status = status_code,
            duration_ms = duration_ms,
            error = error.as_deref().unwrap_or(""),
            "http request completed"
        );
    } else {
        tracing::info!(
            target: "http",
            method = %method,
            url = %url,
            status = status_code,
            duration_ms = duration_ms,
            "http request completed"
        );
    }
}

fn http_request_duration() -> &'static Histogram<f64> {
    static HTTP_REQUEST_DURATION: OnceLock<Histogram<f64>> = OnceLock::new();
    HTTP_REQUEST_DURATION.get_or_init(|| {
        global::meter("qrud-http")
            .f64_histogram("http.server.request.duration")
            .with_description("HTTP server request duration")
            .with_unit("s")
            .build()
    })
}

fn http_errors_total() -> &'static Counter<u64> {
    static HTTP_ERRORS_TOTAL: OnceLock<Counter<u64>> = OnceLock::new();
    HTTP_ERRORS_TOTAL.get_or_init(|| {
        global::meter("qrud-http")
            .u64_counter("http.server.error.count")
            .with_description("HTTP server error responses")
            .build()
    })
}

fn normalize_route(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn extract_error(body_text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body_text).ok()?;
    value
        .get("error")
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_string)
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
