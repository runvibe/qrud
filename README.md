# qrud

`qrud` exists to solve a very practical problem: getting an HTTP API online in minutes, with real CRUD semantics, persistent storage, and enough flexibility to evolve without the weight of a full backend on day one.

That is what makes `qrud` so strong. It is not just a mock server. It gives product teams, frontend teams, and integration flows an incredibly efficient starting point: it boots fast, stores real JSON data, isolates data by workspace, accepts an OpenAPI contract, exposes observability, and runs on either SQLite or Postgres. It is the kind of tool that removes friction and gives development speed back to the team.

## Features

- Ready-to-use HTTP API with CRUD semantics.
- Persistence with SQLite or Postgres.
- Multi-tenant model through `workspace`, with support for `x-workspace-id`.
- Automatic creation of the `default` workspace.
- `--use-default` to simplify local environments and demos.
- Support for any JSON payload.
- Collection listing with `term`, `limit`, `offset`, `order`, and `by`.
- Optional OpenAPI contract validation for routes and payloads.
- `GET /openapi.json` to inspect the active contract.
- CORS configurable through CLI flags or environment variables.
- Optional OpenTelemetry tracing.
- Configuration through flags or `QRUD_*` environment variables.

## Install with Docker

The current `Dockerfile` packages a prebuilt binary. That means the correct flow is:

1. build the `release` binary;
2. build the image;
3. run the container and configure `qrud` through environment variables.

Build the binary:

```bash
cargo build --release
```

Build the image:

```bash
docker build -t qrud .
```

Run with in-memory SQLite:

```bash
docker run --rm -p 3000:3000 \
  -e QRUD_SQLITE=:memory: \
  -e QRUD_USE_DEFAULT=true \
  qrud
```

If you want file persistence:

```bash
docker run --rm -p 3000:3000 \
  -e QRUD_SQLITE=/data/qrud.db \
  -e QRUD_USE_DEFAULT=true \
  -v "$(pwd)/data:/data" \
  qrud
```

If you want Postgres:

```bash
docker run --rm -p 3000:3000 \
  -e QRUD_POSTGRES='postgres://user:pass@host.docker.internal:5432/qrud' \
  qrud
```

There is also `Dockerfile.artifact`, designed for setups where the binary is already separated into `artifacts/<arch>/<app_name>`:

```bash
mkdir -p artifacts/amd64
cp target/release/qrud artifacts/amd64/qrud

docker build -f Dockerfile.artifact \
  --build-arg TARGETARCH=amd64 \
  --build-arg APP_NAME=qrud \
  -t qrud:artifact .

docker run --rm -p 3000:3000 \
  -e QRUD_SQLITE=:memory: \
  qrud:artifact
```

## Install with Binaries

If you prefer to run it without `cargo run`, build the binary and place it on your `PATH`.

Build the executable:

```bash
cargo build --release
```

Install it for the current user:

```bash
mkdir -p ~/.local/bin
cp target/release/qrud ~/.local/bin/qrud
chmod +x ~/.local/bin/qrud
```

Run the binary:

```bash
qrud --port 3000 --sqlite
```

Or with a file-based database:

```bash
qrud --port 3000 --sqlite ./qrud.db --use-default
```

Or with Postgres:

```bash
qrud --port 3000 --postgres "postgres://user:pass@localhost:5432/qrud"
```

## Run with Cargo

For local development, this is the most direct path.

In-memory SQLite:

```bash
cargo run -- --port 3000 --sqlite
```

In-memory SQLite with automatic default workspace:

```bash
cargo run -- --port 3000 --sqlite --use-default
```

SQLite in a file:

```bash
cargo run -- --port 3000 --sqlite ./qrud.db
```

Postgres:

```bash
cargo run -- --port 3000 --postgres "postgres://user:pass@localhost:5432/qrud"
```

CORS configured through CLI:

```bash
cargo run -- --port 3000 --cors \
  --cors-origin http://localhost:5173 \
  --cors-method GET,POST,PUT,PATCH,DELETE,OPTIONS \
  --cors-header content-type,x-workspace-id \
  --cors-credentials true
```

Allow all CORS:

```bash
cargo run -- --port 3000 --cors-allow
```

OpenTelemetry:

```bash
cargo run -- --port 3000 --otel \
  --otel-protocol grpc \
  --otel-endpoint http://localhost:4317 \
  --otel-service-name qrud \
  --otel-service-version 0.1.0 \
  --otel-tracer-name qrud-server \
  --otel-sampler parentbased_traceidratio \
  --otel-sampler-arg 0.25
```

## First Requests

Health check:

```bash
curl http://localhost:3000/health
```

Create a document:

```bash
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Ana","role":"admin"}'
```

List documents:

```bash
curl "http://localhost:3000/users?limit=10&offset=0" \
  -H 'x-workspace-id: default'
```

Upsert with `PUT` and an id in the path:

```bash
curl -X PUT http://localhost:3000/users/7b3a4b2f-5a7e-4a3f-9f4e-8e6a2b0f8e11 \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Bea"}'
```

## Environment Configuration

Every CLI flag can also be configured through a `QRUD_*` environment variable. When both are present, CLI wins.

- `QRUD_HOST`: bind host. Default is `0.0.0.0`.
- `QRUD_PORT`: HTTP port. Default is `3000`.
- `QRUD_SQLITE`: SQLite path or `:memory:`.
- `QRUD_POSTGRES`: Postgres connection URL.
- `QRUD_USE_DEFAULT`: enables the `default` workspace automatically.
- `QRUD_SCHEMA`: OpenAPI contract from file, URL, inline JSON/YAML, or Base64.
- `QRUD_CORS`: enables CORS.
- `QRUD_CORS_ALLOW`: allows origins, methods, and headers with `*`.
- `QRUD_CORS_ORIGINS`: comma-separated list.
- `QRUD_CORS_METHODS`: comma-separated list.
- `QRUD_CORS_HEADERS`: comma-separated list.
- `QRUD_CORS_CREDENTIALS`: enables `Access-Control-Allow-Credentials`.
- `QRUD_OTEL`: enables OpenTelemetry.
- `QRUD_OTEL_ENDPOINT`: OTLP endpoint.
- `QRUD_OTEL_PROTOCOL`: `grpc` or `http`.
- `QRUD_OTEL_SERVICE_NAME`: service name.
- `QRUD_OTEL_SERVICE_VERSION`: reported version.
- `QRUD_OTEL_TRACER_NAME`: tracer name.
- `QRUD_OTEL_SAMPLER`: sampling strategy.
- `QRUD_OTEL_SAMPLER_ARG`: numeric sampler argument.

Example:

```bash
export QRUD_HOST=127.0.0.1
export QRUD_PORT=8080
export QRUD_SQLITE=./data.db
export QRUD_USE_DEFAULT=true
export QRUD_CORS_ALLOW=true
export QRUD_OTEL=true
export QRUD_OTEL_PROTOCOL=grpc
export QRUD_OTEL_ENDPOINT=http://localhost:4317

cargo run
```

## OpenAPI and Contracts

`qrud` can start with an OpenAPI contract and use that contract to validate routes and payloads. That is especially useful when the goal is to simulate an API with more discipline without giving up speed.

Inspect the active contract:

```bash
curl http://localhost:3000/openapi.json
```

Load a contract at runtime:

```bash
curl -X PUT http://localhost:3000/openapi/contract \
  -H 'Content-Type: application/json' \
  -d '{"openapi":"3.0.3","info":{"title":"demo","version":"1.0.0"},"paths":{}}'
```

Remove the active contract:

```bash
curl -X DELETE http://localhost:3000/openapi/contract
```

Start with a local file contract:

```bash
cargo run -- --port 3000 --sqlite --schema ./example.yaml
```

With a remote URL:

```bash
cargo run -- --port 3000 --sqlite --schema https://example.com/openapi.json
```

With inline JSON:

```bash
cargo run -- --port 3000 --sqlite --schema '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}'
```

With Base64:

```bash
SCHEMA_B64=$(printf '%s' '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}' | base64)
cargo run -- --port 3000 --sqlite --schema "$SCHEMA_B64"
```

Only local `#/` references are supported inside the contract.

## Concepts and Endpoints

A `workspace` is the data namespace. Its name must be unique and use `dash-case`. When the database is empty, `default` is created automatically. With `--use-default`, the `x-workspace-id` header becomes optional.

A `document` is any JSON value stored under a path key (`pk`), such as `/users`, `/orders/2024`, or any other structure that fits your domain.

Main endpoints:

- `GET /health`: returns `200 OK`.
- `GET /info`: reports details about the connected database.
- `GET /openapi.json`: returns the current specification.
- `POST /workspaces`: creates a workspace.
- `GET /workspaces`: lists active workspaces.
- `GET /workspaces/{workspace}`: fetches a workspace.
- `PUT /workspaces/{workspace}`: updates name and description.
- `PATCH /workspaces/{workspace}`: partially updates it.
- `DELETE /workspaces/{workspace}`: performs a soft delete.

Document routes work in two formats:

- Header-based: `/{*pk}` with `x-workspace-id: <workspace>`.
- Path-based: `/workspaces/{workspace}/{*pk}`.

Main rules:

- `POST` creates a document and ignores `id` in the payload.
- `PUT` performs an upsert. If the path ends with a UUID, that UUID becomes the document id.
- `PATCH` performs a shallow merge at the root level and requires a JSON object.
- `DELETE` returns `204` when the document exists and `404` when it does not.

Collection listings support:

- `term`: case-insensitive search.
- `limit` and `offset`: pagination.
- `order`: `asc` or `desc`.
- `by`: `created_at` or `updated_at`.

Every returned document includes:

- `$id`
- `$createdAt`
- `$updatedAt`
- `$deletedAt`, when present
- `value`, when the stored payload is not an object

## Logs

To debug requests and responses in more detail:

```bash
RUST_LOG=debug cargo run -- --port 3000 --sqlite
```
