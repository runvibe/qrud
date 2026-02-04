mod models;
mod routes;
mod services;

use std::net::SocketAddr;

use clap::Parser;

use crate::routes::router;
use crate::services::{ApiContract, AppState, Store};

const DEFAULT_PORT: u16 = 3000;

#[derive(Parser, Debug)]
#[command(name = "qrud", about = "HTTP mock server with CRUD semantics")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
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
    #[arg(long, default_value_t = false)]
    use_default: bool,
    #[arg(long, value_name = "FILE")]
    openapi3: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port)
        .parse()
        .expect("invalid host/port");

    let store = match cli.postgres {
        Some(url) => Store::open_postgres(&url)
            .await
            .expect("failed to open postgres db"),
        None => {
            let path = cli.sqlite.as_deref().unwrap_or(":memory:");
            Store::open_sqlite(path)
                .await
                .expect("failed to open sqlite db")
        }
    };
    let api_contract = cli
        .openapi3
        .as_deref()
        .map(ApiContract::from_file)
        .transpose()
        .expect("failed to load openapi3 file");

    let state = AppState::new(store, cli.use_default, api_contract);

    let app = router(state);

    println!("qrud listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app)
        .await
        .expect("server error");
}
