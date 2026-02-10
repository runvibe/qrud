mod documents;
mod mappers;
mod meta;
mod migrations;
mod util;
mod workspaces;

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{PgPool, SqlitePool};
use url::Url;

#[derive(Clone)]
enum Backend {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

#[derive(Clone)]
pub struct Store {
    backend: Backend,
    info: DatabaseInfo,
}

#[derive(Clone, Debug, Serialize)]
pub struct DatabaseInfo {
    pub drive: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite: Option<SqliteInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgres: Option<PostgresInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SqliteInfo {
    pub in_memory: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PostgresInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

impl Store {
    pub async fn open_sqlite(path: &str) -> Result<Self, String> {
        let options = if path == ":memory:" {
            SqliteConnectOptions::new()
                .in_memory(true)
                .shared_cache(true)
                .foreign_keys(true)
        } else {
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true)
        };

        let pool_options = if path == ":memory:" {
            // In-memory SQLite is tied to connection lifetime; recycling the
            // connection would drop all tables and data.
            SqlitePoolOptions::new()
                .max_connections(1)
                .min_connections(1)
                .idle_timeout(None)
                .max_lifetime(None)
        } else {
            SqlitePoolOptions::new().max_connections(1)
        };

        let pool = pool_options
            .connect_with(options)
            .await
            .map_err(|err| err.to_string())?;
        migrations::migrate_sqlite(&pool).await?;
        let info = DatabaseInfo {
            drive: "sqlite",
            sqlite: Some(SqliteInfo {
                in_memory: path == ":memory:",
                path: if path == ":memory:" {
                    None
                } else {
                    Some(path.to_string())
                },
            }),
            postgres: None,
        };
        let store = Self {
            backend: Backend::Sqlite(pool),
            info,
        };
        store.ensure_default_workspace().await?;
        Ok(store)
    }

    pub async fn open_postgres(url: &str) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|err| err.to_string())?;
        migrations::migrate_postgres(&pool).await?;
        let info = DatabaseInfo {
            drive: "postgres",
            sqlite: None,
            postgres: Some(parse_postgres_info(url)),
        };
        let store = Self {
            backend: Backend::Postgres(pool),
            info,
        };
        store.ensure_default_workspace().await?;
        Ok(store)
    }

    pub fn database_info(&self) -> &DatabaseInfo {
        &self.info
    }
}

fn parse_postgres_info(url: &str) -> PostgresInfo {
    let parsed = Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|value| value.host_str())
        .map(|value| value.to_string());
    let port = parsed.as_ref().and_then(|value| value.port());
    let database = parsed
        .as_ref()
        .and_then(|value| value.path_segments())
        .and_then(|mut segments| segments.next_back())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    PostgresInfo {
        host,
        port,
        database,
    }
}
