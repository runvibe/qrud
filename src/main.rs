mod models;
mod routes;
mod services;

use std::net::SocketAddr;

use clap::Parser;

use crate::routes::router;
use crate::services::{AppState, Store};

const DEFAULT_PORT: u16 = 3000;

#[derive(Parser, Debug)]
#[command(name = "qrud", about = "HTTP mock server with CRUD semantics")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
    #[arg(long, default_value = ":memory:")]
    db: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port)
        .parse()
        .expect("invalid host/port");

    let store = Store::open(&cli.db).expect("failed to open sqlite db");
    let state = AppState::new(store);

    let app = router(state);

    println!("qrud listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app)
        .await
        .expect("server error");
}
