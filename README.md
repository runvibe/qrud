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

## OpenAPI

```bash
curl http://localhost:3000/openapi.json
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
