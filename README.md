# sqlc-gen-rust

https://github.com/sqlc-dev/sqlc Rust plugin to generate https://github.com/launchbadge/sqlx models and queries from SQL migrations, schema, queries.

```yml
version: "2"
plugins:
  - name: rust
    wasm:
      url: https://github.com/seriousben/sqlc-gen-rust/releases/download/V{VERSION}/sqlc-gen-RUST.wasm
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


## Roadmap to next release

### Features

- [ ] Support for all sqlc cmd types
- [ ] First release
- [ ] High level documentation


### Chores

- [ ] GitHub Actions to test and manage releases
- [ ] Make examples runnable and testable

## Future

- SQLite support
- MySQL support
- Create structs representing full tables
- Use table structs in params and return values instead of using one off structts
- Support customizing async runtime
- Allow customizing data types
- Allow renaming and overriding types
- Make SQL Queries public
- Improve codegen comments
