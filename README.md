# qrud

Servidor HTTP de mock com semantica CRUD. Os dados sao armazenados em SQLite.

## Como rodar

```bash
cargo run -- --port 3000 --db :memory:
```

Para persistir em arquivo:

```bash
cargo run -- --port 3000 --db ./qrud.db
```

## Rotas

### Criar

```bash
curl -X POST http://localhost:3000/users \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ana","description":"Admin"}'
```

Retorno: `201` com o objeto criado (id sempre auto-incremental).

### Listar com filtros e paginacao

```bash
curl "http://localhost:3000/users?term=car&filter=name&filter=description&filter=tag&limit=10&offset=20"
```

- `term` e case-insensitive.
- `filter` pode repetir para indicar quais campos buscar.
- Se `filter` nao existir, busca em `name`, `title`, `label`, `description`, `category`.
- `limit` e `offset` paginam o resultado.

### Obter item

```bash
curl http://localhost:3000/users/1
```

### Atualizar (PUT)

```bash
curl -X PUT http://localhost:3000/users/1 \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ana","description":"Admin"}'
```

Retorno: `200` se existe, `201` se criou.

### Atualizar parcialmente (PATCH)

```bash
curl -X PATCH http://localhost:3000/users/1 \
  -H 'Content-Type: application/json' \
  -d '{"description":"Admin senior"}'
```

Merge superficial de objetos. `id` no payload e ignorado.

### Remover

```bash
curl -X DELETE http://localhost:3000/users/1
```

Retorno: `204`.
# qrud
