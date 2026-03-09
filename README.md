# qrud

HTTP mock server with CRUD semantics. Data is stored in SQLite or Postgres.

## Running

SQLite in memory (default):

```bash
cargo run -- --port 3000 --sqlite
```

SQLite in memory with automatic default workspace:

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

OpenTelemetry (disabled by default):

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

## Docker Image (Artifact)

`Dockerfile.artifact` expects a prebuilt binary at `artifacts/<arch>/<app_name>` and also uses `APP_NAME` at runtime.

Example for `amd64`:

```bash
mkdir -p artifacts/amd64
cp target/debug/qrud artifacts/amd64/qrud
docker build -f Dockerfile.artifact \
  --build-arg TARGETARCH=amd64 \
  --build-arg APP_NAME=qrud \
  -t qrud:artifact .
docker run --rm -p 3000:3000 qrud:artifact
```

## Environment Variables

Every CLI flag can also be configured through `QRUD_*` environment variables. CLI flags take precedence over environment variables.

- `QRUD_HOST` - Bind host (default: `0.0.0.0`)
- `QRUD_PORT` - HTTP port (default: `3000`)
- `QRUD_SQLITE` - SQLite file path (or `:memory:`)
- `QRUD_POSTGRES` - PostgreSQL connection URL
- `QRUD_USE_DEFAULT` - Enable automatic default workspace (`true` or `false`)
- `QRUD_SCHEMA` - OpenAPI contract source (`file`, `URL`, inline JSON/YAML, or Base64)
- `QRUD_CORS` - Enable CORS (`true` or `false`)
- `QRUD_CORS_ALLOW` - Allow all CORS values (origins, methods, and headers become `*`)
- `QRUD_CORS_ORIGINS` - Comma-separated list of origins (or `*`)
- `QRUD_CORS_METHODS` - Comma-separated list of methods (or `*`)
- `QRUD_CORS_HEADERS` - Comma-separated list of headers (or `*`)
- `QRUD_CORS_CREDENTIALS` - Enable `Access-Control-Allow-Credentials` (`true` or `false`)
- `QRUD_OTEL` - Enable OpenTelemetry (`true` or `false`)
- `QRUD_OTEL_ENDPOINT` - OTLP endpoint (for example `http://localhost:4317` or `http://localhost:4318/v1/traces`)
- `QRUD_OTEL_PROTOCOL` - `grpc` or `http`
- `QRUD_OTEL_SERVICE_NAME` - Service name reported to OTEL
- `QRUD_OTEL_SERVICE_VERSION` - Service version reported to OTEL
- `QRUD_OTEL_TRACER_NAME` - Tracer name
- `QRUD_OTEL_SAMPLER` - `always_on`, `always_off`, `traceidratio`, `parentbased_always_on`, `parentbased_always_off`, `parentbased_traceidratio`
- `QRUD_OTEL_SAMPLER_ARG` - Numeric sampler argument (`0.0` to `1.0` for ratio-based samplers)

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

Mixing CLI and env values (CLI wins):

```bash
export QRUD_PORT=5000
cargo run -- --port 3000  # uses 3000
```

### CLI to Env Mapping

- `--host` <-> `QRUD_HOST`
- `--port` <-> `QRUD_PORT`
- `--sqlite` <-> `QRUD_SQLITE`
- `--postgres` <-> `QRUD_POSTGRES`
- `--use-default` <-> `QRUD_USE_DEFAULT`
- `--schema` <-> `QRUD_SCHEMA`
- `--cors` <-> `QRUD_CORS`
- `--cors-allow` <-> `QRUD_CORS_ALLOW`
- `--cors-origin` <-> `QRUD_CORS_ORIGINS`
- `--cors-method` <-> `QRUD_CORS_METHODS`
- `--cors-header` <-> `QRUD_CORS_HEADERS`
- `--cors-credentials` <-> `QRUD_CORS_CREDENTIALS`
- `--otel` <-> `QRUD_OTEL`
- `--otel-endpoint` <-> `QRUD_OTEL_ENDPOINT`
- `--otel-protocol` <-> `QRUD_OTEL_PROTOCOL`
- `--otel-service-name` <-> `QRUD_OTEL_SERVICE_NAME`
- `--otel-service-version` <-> `QRUD_OTEL_SERVICE_VERSION`
- `--otel-tracer-name` <-> `QRUD_OTEL_TRACER_NAME`
- `--otel-sampler` <-> `QRUD_OTEL_SAMPLER`
- `--otel-sampler-arg` <-> `QRUD_OTEL_SAMPLER_ARG`

## OpenAPI

```bash
curl http://localhost:3000/openapi.json
```

Load or remove a contract at runtime:

```bash
curl -X PUT http://localhost:3000/openapi/contract \
  -H 'Content-Type: application/json' \
  -d '{"openapi":"3.0.3","info":{"title":"demo","version":"1.0.0"},"paths":{}}'

curl -X DELETE http://localhost:3000/openapi/contract
```

## Detailed Documentation

### Concepts

A workspace is the data namespace (multi-tenant boundary). Its name must be unique and use `dash-case`. If the database is empty, the `default` workspace is created automatically. With `--use-default`, the `x-workspace-id` header becomes optional.

A document is any JSON value stored under a path key (`pk`). The `pk` may contain multiple segments, for example `/users` or `/orders/2024`.

### Main Endpoints

- `GET /health` returns `200` with `OK`.
- `GET /info` returns information about the connected database.
- `GET /openapi.json` returns the current OpenAPI specification.

### Workspaces

- `POST /workspaces` creates a workspace. The name must be `dash-case`.
- `GET /workspaces` lists active workspaces.
- `GET /workspaces/{workspace}` fetches a workspace.
- `PUT /workspaces/{workspace}` updates the name and description.
- `PATCH /workspaces/{workspace}` partially updates the name or description.
- `DELETE /workspaces/{workspace}` performs a soft delete and returns `204` when the workspace exists.

### Documents

Routes support two formats:

- Header-based: `/{*pk}` with `x-workspace-id: <workspace_name>`.
- Path-based: `/workspaces/{workspace}/{*pk}`.

Core rules:

- `POST` creates a document and ignores `id` in the payload. If the path ends with a UUID, the request fails.
- `PUT` performs an upsert: it creates when missing and updates when present. If the path ends with a UUID, that UUID is used as the document id.
- `PATCH` performs a shallow merge at the root level only. It requires a JSON object and ignores `id`.
- `DELETE` returns `204` when the document exists and `404` otherwise.

`pk` cannot use reserved values such as `health`, `heath`, `info`, `workspaces`, or `documents`.

### Listing, Search, and Sorting

Collection `GET` requests (when the path does not end with a UUID) support:

- `term`: case-insensitive search across `name`, `title`, `label`, `reference`, `category`, and `description`.
- `limit` and `offset`: pagination.
- `order`: `asc` or `desc` (default `desc`).
- `by`: `created_at` or `updated_at` (default `created_at`).

Responses include `items`, `total`, `limit`, `offset`, `order`, and `by`.

### Output Format

Every returned document includes:

- `$id`, `$createdAt`, and `$updatedAt`.
- `$deletedAt` when present.
- `value` when the stored payload is not an object.

### OpenAPI Contract (Optional)

When the server starts with `--schema <source>` or `QRUD_SCHEMA=<source>`, it validates:

- Routes: if the route does not exist in the contract, it returns `404`.
- Payloads: if the payload does not match the schema, it returns `400`.

You can also load or replace the active contract at runtime with `PUT /openapi/contract` by sending the OpenAPI JSON document in the request body. `DELETE /openapi/contract` removes the current contract.

Only local references (`#/`) are supported inside the contract.

### Logs

With `RUST_LOG=debug`, the server logs requests and responses, including headers and body.

## Use Cases

1. Quick frontend mock without a real backend

```bash
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Ana","role":"admin"}'

curl "http://localhost:3000/users?limit=10&offset=0" \
  -H 'x-workspace-id: default'
```

2. Multi-tenant usage with workspace in the path

```bash
curl -X POST http://localhost:3000/workspaces/acme-inc \
  -H 'Content-Type: application/json' \
  -d '{"name":"acme-inc"}'

curl -X POST http://localhost:3000/workspaces/acme-inc/orders \
  -H 'Content-Type: application/json' \
  -d '{"total": 120.5, "status":"paid"}'
```

3. Search and ordering in collections

```bash
curl "http://localhost:3000/products?term=shoe&order=asc&by=updated_at&limit=5&offset=0" \
  -H 'x-workspace-id: default'
```

4. Upsert with an id in the path

```bash
curl -X PUT http://localhost:3000/users/7b3a4b2f-5a7e-4a3f-9f4e-8e6a2b0f8e11 \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Bea"}'
```

5. Validation through an OpenAPI contract

```bash
# local file
cargo run -- --port 3000 --sqlite --schema ./example.yaml

# remote URL
cargo run -- --port 3000 --sqlite --schema https://example.com/openapi.json

# inline JSON
cargo run -- --port 3000 --sqlite --schema '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}'

# Base64 (encoded JSON or YAML content)
SCHEMA_B64=$(printf '%s' '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}' | base64)
cargo run -- --port 3000 --sqlite --schema "$SCHEMA_B64"
```

## Routes

### Workspaces

Workspace names must be unique and use `dash-case`. On first startup, the `default` workspace is created automatically.

```bash
curl -X POST http://localhost:3000/workspaces \
  -H 'Content-Type: application/json' \
  -d '{"name":"main","description":"Team workspace"}'
```

```bash
curl http://localhost:3000/workspaces
```

```bash
curl http://localhost:3000/workspaces/<workspace_name>
```

```bash
curl -X PUT http://localhost:3000/workspaces/<workspace_name> \
  -H 'Content-Type: application/json' \
  -d '{"name":"main","description":"Updated"}'
```

```bash
curl -X PATCH http://localhost:3000/workspaces/<workspace_name> \
  -H 'Content-Type: application/json' \
  -d '{"description":"Ops"}'
```

```bash
curl -X DELETE http://localhost:3000/workspaces/<workspace_name>
```

### Documents via Header

```bash
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_name>' \
  -d '{"name":"Ana"}'
```

```bash
curl http://localhost:3000/users \
  -H 'x-workspace-id: <workspace_name>'
```

```bash
curl -X PUT http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_name>' \
  -d '{"name":"Bea"}'
```

```bash
curl -X PATCH http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_name>' \
  -d '{"role":"admin"}'
```

```bash
curl -X DELETE http://localhost:3000/users \
  -H 'x-workspace-id: <workspace_name>'
```

### Documents via Workspace Path

```bash
curl -X POST http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"title":"Hello"}'
```

```bash
curl http://localhost:3000/workspaces/<workspace_name>/posts
```

```bash
curl -X PUT http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"title":"New"}'
```

```bash
curl -X PATCH http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"status":"ok"}'
```

```bash
curl -X DELETE http://localhost:3000/workspaces/<workspace_name>/posts
```
