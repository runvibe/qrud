# qrud

Servidor HTTP de mock com semantica CRUD. Os dados sao armazenados em SQLite ou Postgres.

## Como rodar

SQLite em memoria (padrao):

```bash
cargo run -- --port 3000 --sqlite
```

SQLite em memoria com workspace default automatico:

```bash
cargo run -- --port 3000 --sqlite --use-default
```

SQLite em arquivo:

```bash
cargo run -- --port 3000 --sqlite ./qrud.db
```

Postgres:

```bash
cargo run -- --port 3000 --postgres "postgres://user:pass@localhost:5432/qrud"
```

CORS configurado pela CLI:

```bash
cargo run -- --port 3000 --cors \
  --cors-origin http://localhost:5173 \
  --cors-method GET,POST,PUT,PATCH,DELETE,OPTIONS \
  --cors-header content-type,x-workspace-id \
  --cors-credentials true
```

Liberar todos os CORS:

```bash
cargo run -- --port 3000 --cors-allow
```

OpenTelemetry (desativado por padrao):

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

## Imagem Docker (artifact)

O `Dockerfile.artifact` espera um binario precompilado em `artifacts/<arch>/<app_name>` e usa `APP_NAME` tambem em runtime.

Exemplo para `amd64`:

```bash
mkdir -p artifacts/amd64
cp target/debug/qrud artifacts/amd64/qrud
docker build -f Dockerfile.artifact \
  --build-arg TARGETARCH=amd64 \
  --build-arg APP_NAME=qrud \
  -t qrud:artifact .
docker run --rm -p 3000:3000 qrud:artifact
```

## Configuração via variáveis de ambiente

Todas as flags da CLI podem ser configuradas via variáveis de ambiente com o prefixo `QRUD_*`. A CLI tem prioridade sobre as variáveis de ambiente.

- `QRUD_HOST` — Host para bind (default: `0.0.0.0`)
- `QRUD_PORT` — Porta HTTP (default: `3000`)
- `QRUD_SQLITE` — Caminho para arquivo SQLite (ou `:memory:`)
- `QRUD_POSTGRES` — URL de conexão PostgreSQL
- `QRUD_USE_DEFAULT` — Usa workspace default automático (`true` ou `false`)
- `QRUD_SCHEMA` — Fonte do contrato OpenAPI (`arquivo`, `URL`, JSON/YAML inline ou Base64)
- `QRUD_CORS` — Habilita CORS (`true` ou `false`)
- `QRUD_CORS_ALLOW` — Libera CORS total (origins, methods, headers com `*`)
- `QRUD_CORS_ORIGINS` — Lista de origins separados por vírgula (ou `*`)
- `QRUD_CORS_METHODS` — Lista de métodos separados por vírgula (ou `*`)
- `QRUD_CORS_HEADERS` — Lista de headers separados por vírgula (ou `*`)
- `QRUD_CORS_CREDENTIALS` — Habilita `Access-Control-Allow-Credentials` (`true` ou `false`)
- `QRUD_OTEL` — Habilita OpenTelemetry (`true` ou `false`)
- `QRUD_OTEL_ENDPOINT` — Endpoint OTLP (ex.: `http://localhost:4317` ou `http://localhost:4318/v1/traces`)
- `QRUD_OTEL_PROTOCOL` — `grpc` ou `http`
- `QRUD_OTEL_SERVICE_NAME` — Nome do servico reportado no OTEL
- `QRUD_OTEL_SERVICE_VERSION` — Versao do servico reportada no OTEL
- `QRUD_OTEL_TRACER_NAME` — Nome do tracer
- `QRUD_OTEL_SAMPLER` — `always_on`, `always_off`, `traceidratio`, `parentbased_always_on`, `parentbased_always_off`, `parentbased_traceidratio`
- `QRUD_OTEL_SAMPLER_ARG` — argumento numerico do sampler (0.0 a 1.0 para ratio)

Exemplo:

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

Mix de CLI e env (CLI sobrescreve env):

```bash
export QRUD_PORT=5000
cargo run -- --port 3000  # usa 3000
```

### Mapa CLI para env

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

## Documentacao detalhada

### Conceitos

Workspace e o namespace (multi-tenant) do dado. O nome deve ser `dash-case` e unico. Se o banco estiver vazio, o workspace `default` e criado automaticamente. Com `--use-default`, o header `x-workspace-id` passa a ser opcional.

Documento e qualquer JSON armazenado sob uma chave de path (`pk`). O `pk` pode ter mais de um segmento, por exemplo `/users` ou `/orders/2024`.

### Endpoints principais

- `GET /health` retorna `200` com "OK".
- `GET /info` retorna informacoes do banco conectado.
- `GET /openapi.json` retorna a especificacao OpenAPI gerada.

### Workspaces

- `POST /workspaces` cria workspace. Nome deve ser `dash-case`.
- `GET /workspaces` lista workspaces ativos.
- `GET /workspaces/{workspace}` busca workspace.
- `PUT /workspaces/{workspace}` atualiza nome e descricao.
- `PATCH /workspaces/{workspace}` atualiza parcialmente nome ou descricao.
- `DELETE /workspaces/{workspace}` faz soft delete e retorna `204` se existir.

### Documentos

As rotas aceitam dois formatos:

- Via header: `/{*pk}` com `x-workspace-id: <workspace_name>`.
- Via path: `/workspaces/{workspace}/{*pk}`.

Regras principais:

- `POST` cria e ignora `id` no payload. Se o path terminar com UUID, retorna erro.
- `PUT` faz upsert: cria se nao existir e atualiza se existir. Se o path terminar com UUID, usa como id.
- `PATCH` faz merge superficial apenas no nivel raiz. Exige JSON object e ignora `id`.
- `DELETE` retorna `204` se existir e `404` caso contrario.

O `pk` nao pode ser reservado para `health`, `heath`, `info`, `workspaces`, `documents`.

### Listagem, busca e ordenacao

`GET` em colecao (quando o path nao termina com UUID) suporta:

- `term`: busca case-insensitive nos campos `name`, `title`, `label`, `reference`, `category`, `description`.
- `limit` e `offset`: paginacao.
- `order`: `asc` ou `desc` (padrao `desc`).
- `by`: `created_at` ou `updated_at` (padrao `created_at`).

Resposta inclui: `items`, `total`, `limit`, `offset`, `order`, `by`.

### Formato de saida

Todo documento retornado inclui:

- `$id`, `$createdAt`, `$updatedAt`.
- `$deletedAt` quando existe.
- Se o payload armazenado nao for objeto, o valor volta em `value`.

### Contrato OpenAPI (opcional)

Ao iniciar com `--schema <fonte>` ou `QRUD_SCHEMA=<fonte>`, o servidor valida:

- Rotas: se a rota nao existir no contrato, retorna `404`.
- Payload: se o payload nao bater no schema, retorna `400`.

Somente referencias locais (`#/`) sao suportadas no contrato.

### Logs

Com `RUST_LOG=debug`, o servidor registra request e response (headers e body) no log.

## Casos de uso

1. Mock rapido para frontend sem backend real

```bash
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Ana","role":"admin"}'

curl "http://localhost:3000/users?limit=10&offset=0" \
  -H 'x-workspace-id: default'
```

2. Multi-tenant com workspace no path

```bash
curl -X POST http://localhost:3000/workspaces/acme-inc \
  -H 'Content-Type: application/json' \
  -d '{"name":"acme-inc"}'

curl -X POST http://localhost:3000/workspaces/acme-inc/orders \
  -H 'Content-Type: application/json' \
  -d '{"total": 120.5, "status":"paid"}'
```

3. Busca e ordenacao em colecoes

```bash
curl "http://localhost:3000/products?term=shoe&order=asc&by=updated_at&limit=5&offset=0" \
  -H 'x-workspace-id: default'
```

4. Upsert com id no path

```bash
curl -X PUT http://localhost:3000/users/7b3a4b2f-5a7e-4a3f-9f4e-8e6a2b0f8e11 \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: default' \
  -d '{"name":"Bea"}'
```

5. Validacao por contrato OpenAPI

```bash
# arquivo local
cargo run -- --port 3000 --sqlite --schema ./schema.json

# URL remota
cargo run -- --port 3000 --sqlite --schema https://example.com/openapi.json

# JSON inline
cargo run -- --port 3000 --sqlite --schema '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}'

# Base64 (conteudo JSON/YAML codificado)
SCHEMA_B64=$(printf '%s' '{"openapi":"3.0.3","info":{"title":"x","version":"1"},"paths":{}}' | base64)
cargo run -- --port 3000 --sqlite --schema "$SCHEMA_B64"
```

## Rotas

### Workspaces

O nome do workspace deve ser `dash-case` e unico. Na primeira inicializacao, o workspace `default` e criado automaticamente.

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

### Documents via header

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

### Documents via workspace no path

```bash
curl -X POST http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"title":"Oi"}'
```

```bash
curl http://localhost:3000/workspaces/<workspace_name>/posts
```

```bash
curl -X PUT http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"title":"Novo"}'
```

```bash
curl -X PATCH http://localhost:3000/workspaces/<workspace_name>/posts \
  -H 'Content-Type: application/json' \
  -d '{"status":"ok"}'
```

```bash
curl -X DELETE http://localhost:3000/workspaces/<workspace_name>/posts
```
