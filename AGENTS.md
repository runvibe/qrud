# AGENTS.md

Diretrizes de desenvolvimento do projeto `qrud`.

**Workflow**
1. Mantenha mudancas pequenas e focadas por tarefa.
2. Sempre execute `cargo test` ao finalizar uma tarefa.
3. Se nao rodar testes, explique o motivo no resumo da entrega.

**Arquitetura**
- `src/main.rs` apenas CLI e bootstrap do servidor.
- `src/routes/` para handlers e parsing de request.
- `src/services/` para acesso ao SQLite e logica de persistencia.
- `src/models.rs` para modelos de entrada e constantes.

**Regras de API**
- `POST /{colecao}` sempre ignora `id` do payload e auto-incrementa.
- `PUT /{colecao}/{id}` cria se nao existir e atualiza o contador.
- `PATCH /{colecao}/{id}` faz merge superficial e ignora `id` do payload.
- `DELETE /{colecao}/{id}` retorna `204` se existir, `404` caso contrario.
- `GET /{colecao}` suporta filtros e paginacao.

**Filtros e paginacao**
- `term` e case-insensitive.
- `filter` pode repetir para indicar campos consultados.
- Sem `filter`, usa `name`, `title`, `label`, `description`, `category`.
- `limit` e `offset` paginam o resultado.

**Persistencia**
- SQLite `v0.37.0` e storage em `:memory:` por padrao.
**Fim**
