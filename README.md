# sqlc-gen-rust

https://github.com/sqlc-dev/sqlc Rust plugin to generate https://github.com/launchbadge/sqlx models and queries from SQL migrations, schema, queries.

```yml
version: "2"
plugins:
  - name: rust
    wasm:
      url: https://github.com/seriousben/sqlc-gen-rust/releases/download/v{VERSION}/sqlc-gen-rust.wasm
      sha256: {VERSION_SHA256}
sql:
  - schema: "schema/postgresql/schema.sql"
    queries: "schema/postgresql/query.sql"
    engine: "postgresql"
    codegen:
      - plugin: rust
        out: sqlx/src/db
        options:
          driver: "sqlx"
```

## Road to first release

### Features

- [ ] Support for all sqlc cmd types
- [ ] High level documentation
- [ ] Fully working axum-social-with-tests example

### Chores

- [ ] Release tooling
- [ ] GitHub Actions to test and manage releases
- [ ] Make examples runnable and testable

## Future

- Transaction support
- SQLite support
- MySQL support
- Improve codegen comments
- Using structs representing full tables
- Make SQL Queries public
- Support customizing async runtime
- Allow customizing data types
- Allow renaming and overriding types