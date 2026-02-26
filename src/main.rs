mod models;
mod routes;
mod services;

use std::net::SocketAddr;

use axum::http::{HeaderValue, Method, header::HeaderName};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use clap::Parser;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::routes::router;
use crate::services::{ApiContract, AppState, Store};

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_OTEL_PROTOCOL: &str = "grpc";
const DEFAULT_OTEL_SAMPLER: &str = "parentbased_always_on";

#[derive(Parser, Debug)]
#[command(name = "qrud", about = "HTTP mock server with CRUD semantics")]
struct Cli {
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(
        long,
        value_name = "PATH",
        default_missing_value = ":memory:",
        num_args = 0..=1,
        conflicts_with = "postgres"
    )]
    sqlite: Option<String>,
    #[arg(long, value_name = "URL", conflicts_with = "sqlite")]
    postgres: Option<String>,
    #[arg(long)]
    use_default: Option<bool>,
    #[arg(long, value_name = "SOURCE")]
    schema: Option<String>,
    #[arg(long, default_value_t = false)]
    cors: bool,
    #[arg(long, default_value_t = false)]
    cors_allow: bool,
    #[arg(long = "cors-origin", value_name = "ORIGIN", value_delimiter = ',')]
    cors_origin: Vec<String>,
    #[arg(long = "cors-method", value_name = "METHOD", value_delimiter = ',')]
    cors_method: Vec<String>,
    #[arg(long = "cors-header", value_name = "HEADER", value_delimiter = ',')]
    cors_header: Vec<String>,
    #[arg(long)]
    cors_credentials: Option<bool>,
    #[arg(long, default_value_t = false)]
    otel: bool,
    #[arg(long = "otel-endpoint", value_name = "URL")]
    otel_endpoint: Option<String>,
    #[arg(long = "otel-protocol", value_name = "PROTOCOL")]
    otel_protocol: Option<String>,
    #[arg(long = "otel-service-name", value_name = "NAME")]
    otel_service_name: Option<String>,
    #[arg(long = "otel-service-version", value_name = "VERSION")]
    otel_service_version: Option<String>,
    #[arg(long = "otel-tracer-name", value_name = "NAME")]
    otel_tracer_name: Option<String>,
    #[arg(long = "otel-sampler", value_name = "SAMPLER")]
    otel_sampler: Option<String>,
    #[arg(long = "otel-sampler-arg", value_name = "FLOAT")]
    otel_sampler_arg: Option<f64>,
}

#[derive(Debug)]
struct Options {
    host: String,
    port: u16,
    sqlite: Option<String>,
    postgres: Option<String>,
    use_default: bool,
    schema: Option<String>,
    cors_enabled: bool,
    cors_allow_all: bool,
    cors_origins: Vec<String>,
    cors_methods: Vec<String>,
    cors_headers: Vec<String>,
    cors_credentials: bool,
    otel_enabled: bool,
    otel_endpoint: Option<String>,
    otel_protocol: String,
    otel_service_name: String,
    otel_service_version: String,
    otel_tracer_name: String,
    otel_sampler: String,
    otel_sampler_arg: Option<f64>,
}

impl Options {
    fn from_cli_and_env(cli: Cli) -> Self {
        let cli_cors_origins = normalize_list(cli.cors_origin);
        let cli_cors_methods = normalize_list(cli.cors_method);
        let cli_cors_headers = normalize_list(cli.cors_header);
        let env_host = std::env::var("QRUD_HOST").ok();
        let env_port = std::env::var("QRUD_PORT").ok().and_then(|p| p.parse().ok());
        let env_sqlite = std::env::var("QRUD_SQLITE").ok();
        let env_postgres = std::env::var("QRUD_POSTGRES").ok();
        let env_use_default = std::env::var("QRUD_USE_DEFAULT")
            .ok()
            .and_then(|v| v.parse().ok());
        let env_schema = std::env::var("QRUD_SCHEMA").ok();
        let env_cors = std::env::var("QRUD_CORS").ok().and_then(|v| v.parse().ok());
        let env_cors_allow = std::env::var("QRUD_CORS_ALLOW")
            .ok()
            .and_then(|v| v.parse().ok());
        let env_cors_credentials = std::env::var("QRUD_CORS_CREDENTIALS")
            .ok()
            .and_then(|v| v.parse().ok());
        let env_cors_origins = std::env::var("QRUD_CORS_ORIGINS")
            .ok()
            .map(|v| parse_csv_list(&v))
            .unwrap_or_default();
        let env_cors_methods = std::env::var("QRUD_CORS_METHODS")
            .ok()
            .map(|v| parse_csv_list(&v))
            .unwrap_or_default();
        let env_cors_headers = std::env::var("QRUD_CORS_HEADERS")
            .ok()
            .map(|v| parse_csv_list(&v))
            .unwrap_or_default();
        let env_otel = std::env::var("QRUD_OTEL").ok().and_then(|v| v.parse().ok());
        let env_otel_endpoint = std::env::var("QRUD_OTEL_ENDPOINT").ok();
        let env_otel_protocol = std::env::var("QRUD_OTEL_PROTOCOL").ok();
        let env_otel_service_name = std::env::var("QRUD_OTEL_SERVICE_NAME").ok();
        let env_otel_service_version = std::env::var("QRUD_OTEL_SERVICE_VERSION").ok();
        let env_otel_tracer_name = std::env::var("QRUD_OTEL_TRACER_NAME").ok();
        let env_otel_sampler = std::env::var("QRUD_OTEL_SAMPLER").ok();
        let env_otel_sampler_arg = std::env::var("QRUD_OTEL_SAMPLER_ARG")
            .ok()
            .and_then(|v| v.parse().ok());

        let cors_origins = if cli_cors_origins.is_empty() {
            env_cors_origins
        } else {
            cli_cors_origins
        };
        let cors_methods = if cli_cors_methods.is_empty() {
            env_cors_methods
        } else {
            cli_cors_methods
        };
        let cors_headers = if cli_cors_headers.is_empty() {
            env_cors_headers
        } else {
            cli_cors_headers
        };
        let cors_credentials = cli
            .cors_credentials
            .or(env_cors_credentials)
            .unwrap_or(false);
        let cors_allow_all = cli.cors_allow || env_cors_allow.unwrap_or(false);
        let cors_enabled = cli.cors
            || env_cors.unwrap_or(false)
            || cors_allow_all
            || !cors_origins.is_empty()
            || !cors_methods.is_empty()
            || !cors_headers.is_empty()
            || cors_credentials;
        let otel_enabled = cli.otel || env_otel.unwrap_or(false);
        let otel_endpoint = cli.otel_endpoint.or(env_otel_endpoint);
        let otel_protocol = cli
            .otel_protocol
            .or(env_otel_protocol)
            .unwrap_or_else(|| DEFAULT_OTEL_PROTOCOL.to_string());
        let otel_service_name = cli
            .otel_service_name
            .or(env_otel_service_name)
            .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_string());
        let otel_service_version = cli
            .otel_service_version
            .or(env_otel_service_version)
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let otel_tracer_name = cli
            .otel_tracer_name
            .or(env_otel_tracer_name)
            .unwrap_or_else(|| otel_service_name.clone());
        let otel_sampler = cli
            .otel_sampler
            .or(env_otel_sampler)
            .unwrap_or_else(|| DEFAULT_OTEL_SAMPLER.to_string());
        let otel_sampler_arg = cli.otel_sampler_arg.or(env_otel_sampler_arg);

        Self {
            host: cli
                .host
                .or(env_host)
                .unwrap_or_else(|| DEFAULT_HOST.to_string()),
            port: cli.port.or(env_port).unwrap_or(DEFAULT_PORT),
            sqlite: cli.sqlite.or(env_sqlite),
            postgres: cli.postgres.or(env_postgres),
            use_default: cli.use_default.or(env_use_default).unwrap_or(false),
            schema: cli.schema.or(env_schema),
            cors_enabled,
            cors_allow_all,
            cors_origins,
            cors_methods,
            cors_headers,
            cors_credentials,
            otel_enabled,
            otel_endpoint,
            otel_protocol,
            otel_service_name,
            otel_service_version,
            otel_tracer_name,
            otel_sampler,
            otel_sampler_arg,
        }
    }
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|value| parse_csv_list(&value))
        .collect()
}

fn build_cors_layer(opts: &Options) -> Result<Option<CorsLayer>, String> {
    if !opts.cors_enabled {
        return Ok(None);
    }

    if opts.cors_allow_all {
        if opts.cors_credentials {
            return Err(
                "Invalid CORS configuration: --cors-allow cannot be used with credentials"
                    .to_string(),
            );
        }
        return Ok(Some(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        ));
    }

    let mut cors = CorsLayer::new();

    if opts.cors_origins.is_empty() || opts.cors_origins.iter().any(|v| v == "*") {
        cors = cors.allow_origin(Any);
    } else {
        let mut origins = Vec::with_capacity(opts.cors_origins.len());
        for origin in &opts.cors_origins {
            let value = origin.trim();
            let parsed = HeaderValue::from_str(value)
                .map_err(|err| format!("Invalid CORS origin `{value}`: {err}"))?;
            origins.push(parsed);
        }
        cors = cors.allow_origin(origins);
    }

    if opts.cors_methods.is_empty() || opts.cors_methods.iter().any(|v| v == "*") {
        cors = cors.allow_methods(Any);
    } else {
        let mut methods = Vec::with_capacity(opts.cors_methods.len());
        for method in &opts.cors_methods {
            let value = method.trim();
            let parsed = Method::from_bytes(value.as_bytes())
                .map_err(|err| format!("Invalid CORS method `{value}`: {err}"))?;
            methods.push(parsed);
        }
        cors = cors.allow_methods(methods);
    }

    if opts.cors_headers.is_empty() || opts.cors_headers.iter().any(|v| v == "*") {
        cors = cors.allow_headers(Any);
    } else {
        let mut headers = Vec::with_capacity(opts.cors_headers.len());
        for header in &opts.cors_headers {
            let value = header.trim();
            let parsed = HeaderName::from_bytes(value.as_bytes())
                .map_err(|err| format!("Invalid CORS header `{value}`: {err}"))?;
            headers.push(parsed);
        }
        cors = cors.allow_headers(headers);
    }

    if opts.cors_credentials {
        cors = cors.allow_credentials(true);
    }

    Ok(Some(cors))
}

fn normalize_otel_protocol(value: &str) -> Result<Protocol, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "grpc" => Ok(Protocol::Grpc),
        "http" | "http/protobuf" | "http-protobuf" => Ok(Protocol::HttpBinary),
        other => Err(format!(
            "Invalid OTEL protocol `{other}`. Use `grpc` or `http`."
        )),
    }
}

fn parse_otel_sampler(name: &str, arg: Option<f64>) -> Result<Sampler, String> {
    let normalize_ratio = |value: f64| -> Result<f64, String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(
                "Invalid OTEL sampler arg. Expected ratio between 0.0 and 1.0.".to_string(),
            );
        }
        Ok(value)
    };

    match name.trim().to_ascii_lowercase().as_str() {
        "always_on" => Ok(Sampler::AlwaysOn),
        "always_off" => Ok(Sampler::AlwaysOff),
        "traceidratio" => {
            let ratio = normalize_ratio(arg.unwrap_or(1.0))?;
            Ok(Sampler::TraceIdRatioBased(ratio))
        }
        "parentbased_always_on" => Ok(Sampler::ParentBased(Box::new(Sampler::AlwaysOn))),
        "parentbased_always_off" => Ok(Sampler::ParentBased(Box::new(Sampler::AlwaysOff))),
        "parentbased_traceidratio" => {
            let ratio = normalize_ratio(arg.unwrap_or(1.0))?;
            Ok(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                ratio,
            ))))
        }
        other => Err(format!(
            "Invalid OTEL sampler `{other}`. Use: always_on, always_off, traceidratio, parentbased_always_on, parentbased_always_off, parentbased_traceidratio."
        )),
    }
}

fn init_tracing(opts: &Options) -> Result<Option<SdkTracerProvider>, String> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if !opts.otel_enabled {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return Ok(None);
    }

    let protocol = normalize_otel_protocol(&opts.otel_protocol)?;
    let sampler = parse_otel_sampler(&opts.otel_sampler, opts.otel_sampler_arg)?;

    let exporter = match protocol {
        Protocol::Grpc => {
            let mut builder = opentelemetry_otlp::SpanExporter::builder().with_tonic();
            if let Some(endpoint) = opts.otel_endpoint.as_deref() {
                builder = builder.with_endpoint(endpoint.to_string());
            }
            builder.build().map_err(|err| err.to_string())?
        }
        Protocol::HttpBinary => {
            let mut builder = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_protocol(Protocol::HttpBinary);
            if let Some(endpoint) = opts.otel_endpoint.as_deref() {
                builder = builder.with_endpoint(endpoint.to_string());
            }
            builder.build().map_err(|err| err.to_string())?
        }
        _ => {
            return Err("Unsupported OTEL protocol".to_string());
        }
    };

    let resource = Resource::builder()
        .with_service_name(opts.otel_service_name.clone())
        .with_attributes([KeyValue::new(
            "service.version",
            opts.otel_service_version.clone(),
        )])
        .build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(sampler)
        .with_batch_exporter(exporter)
        .build();

    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(tracer_provider.clone());

    let tracer = tracer_provider.tracer(opts.otel_tracer_name.clone());
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(Some(tracer_provider))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let opts = Options::from_cli_and_env(cli);
    let tracer_provider = init_tracing(&opts).expect("failed to initialize tracing");

    let addr: SocketAddr = format!("{}:{}", opts.host, opts.port)
        .parse()
        .expect("invalid host/port");

    let store = match opts.postgres.as_deref() {
        Some(url) => Store::open_postgres(&url)
            .await
            .expect("failed to open postgres db"),
        None => {
            let path = opts.sqlite.as_deref().unwrap_or(":memory:");
            Store::open_sqlite(path)
                .await
                .expect("failed to open sqlite db")
        }
    };
    let api_contract = opts
        .schema
        .as_deref()
        .map(ApiContract::from_file)
        .transpose()
        .expect("failed to load schema source");

    let state = AppState::new(store, opts.use_default, api_contract);

    let app = router(state);
    let app = if opts.otel_enabled {
        app.layer(OtelInResponseLayer::default())
            .layer(OtelAxumLayer::default())
    } else {
        app
    };
    let app = match build_cors_layer(&opts).expect("invalid CORS configuration") {
        Some(cors_layer) => app.layer(cors_layer),
        None => app,
    };

    println!("qrud listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    let server_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await;
    if let Some(provider) = tracer_provider {
        let _ = provider.shutdown();
    }
    server_result.expect("server error");
}
