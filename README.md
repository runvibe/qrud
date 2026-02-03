# qrud

Servidor HTTP de mock com semantica CRUD. Os dados sao armazenados em SQLite ou Postgres.

## Como rodar

SQLite em memoria (padrao):

```bash
cargo run -- --port 3000 --sqlite
```

SQLite em arquivo:

```bash
cargo run -- --port 3000 --sqlite ./qrud.db
```

Postgres:

```bash
cargo run -- --port 3000 --postgres "postgres://user:pass@localhost:5432/qrud"
```

## OpenAPI

```bash
curl http://localhost:3000/openapi.json
```

## Rotas

### Workspaces

```bash
curl -X POST http://localhost:3000/workspaces \
  -H 'Content-Type: application/json' \
  -d '{"name":"Main","description":"Team workspace"}'
```

```bash
curl http://localhost:3000/workspaces
```

```bash
curl http://localhost:3000/workspaces/<workspace_id>
```

```bash
curl -X PUT http://localhost:3000/workspaces/<workspace_id> \
  -H 'Content-Type: application/json' \
  -d '{"name":"Main","description":"Updated"}'
```

```bash
curl -X PATCH http://localhost:3000/workspaces/<workspace_id> \
  -H 'Content-Type: application/json' \
  -d '{"description":"Ops"}'
```

```bash
curl -X DELETE http://localhost:3000/workspaces/<workspace_id>
```

### Documents por workspace

```bash
curl -X POST http://localhost:3000/workspaces/<workspace_id>/documents/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ana"}'
```

```bash
curl http://localhost:3000/workspaces/<workspace_id>/documents/users
```

```bash
curl -X PUT http://localhost:3000/workspaces/<workspace_id>/documents/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Bea"}'
```

```bash
curl -X PATCH http://localhost:3000/workspaces/<workspace_id>/documents/users \
  -H 'Content-Type: application/json' \
  -d '{"role":"admin"}'
```

```bash
curl -X DELETE http://localhost:3000/workspaces/<workspace_id>/documents/users
```

### Documents via header

```bash
curl -X POST http://localhost:3000/documents/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_id>' \
  -d '{"name":"Ana"}'
```

```bash
curl http://localhost:3000/documents/users \
  -H 'x-workspace-id: <workspace_id>'
```

```bash
curl -X PUT http://localhost:3000/documents/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_id>' \
  -d '{"name":"Bea"}'
```

```bash
curl -X PATCH http://localhost:3000/documents/users \
  -H 'Content-Type: application/json' \
  -H 'x-workspace-id: <workspace_id>' \
  -d '{"role":"admin"}'
```

```bash
curl -X DELETE http://localhost:3000/documents/users \
  -H 'x-workspace-id: <workspace_id>'
```
