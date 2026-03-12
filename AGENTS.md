# AGENTS.md

Development guidelines for the `qrud` project.

**Workflow**
1. Keep changes small and focused per task.
2. Always run `cargo test` when finishing a task.
3. If `cargo test` succeeds, commit and push the project.
4. If you do not run tests, explain why in the delivery summary.

**Architecture**
- `src/main.rs` should contain only CLI code and server bootstrap.
- `src/routes/` is for handlers and request parsing.
- `src/services/` is for SQLite access and persistence logic.
- `src/models.rs` is for input models and constants.

**API Rules**
- `POST /{collection}` always ignores `id` from the payload and auto-increments.
- `PUT /{collection}/{id}` creates the record if it does not exist and updates the counter.
- `PATCH /{collection}/{id}` performs a shallow merge and ignores `id` from the payload.
- `DELETE /{collection}/{id}` returns `204` if it exists, `404` otherwise.
- `GET /{collection}` supports filtering and pagination.

**Filtering and Pagination**
- `term` is case-insensitive.
- `filter` can be repeated to indicate which fields are queried.
- Without `filter`, use `name`, `title`, `label`, `description`, `category`.
- `limit` and `offset` paginate the result.

**Persistence**
- SQLite `v0.37.0` and `:memory:` storage by default.
**End**
