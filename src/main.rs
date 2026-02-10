mod models;
mod routes;
mod services;

use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::routes::router;
use crate::services::{ApiContract, AppState, Store};

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_HOST: &str = "0.0.0.0";

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
    #[arg(long, value_name = "FILE")]
    schema: Option<String>,
}

#[derive(Debug)]
struct Options {
    host: String,
    port: u16,
    sqlite: Option<String>,
    postgres: Option<String>,
    use_default: bool,
    schema: Option<String>,
}

impl Options {
    fn from_cli_and_env(cli: Cli) -> Self {
        let env_host = std::env::var("QRUD_HOST").ok();
        let env_port = std::env::var("QRUD_PORT").ok().and_then(|p| p.parse().ok());
        let env_sqlite = std::env::var("QRUD_SQLITE").ok();
        let env_postgres = std::env::var("QRUD_POSTGRES").ok();
        let env_use_default = std::env::var("QRUD_USE_DEFAULT")
            .ok()
            .and_then(|v| v.parse().ok());
        let env_schema = std::env::var("QRUD_SCHEMA").ok();

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
        }
    }
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();
    let opts = Options::from_cli_and_env(cli);
    
    let addr: SocketAddr = format!("{}:{}", opts.host, opts.port)
        .parse()
        .expect("invalid host/port");

    let store = match opts.postgres {
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
        .expect("failed to load schema file");

    let state = AppState::new(store, opts.use_default, api_contract);

    let app = router(state);

    println!("qrud listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("server error");
}
