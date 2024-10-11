# kitchen-sink-riverqueue

https://github.com/riverqueue/river is one of the biggest open source app using sqlc in production right now.

We are borrowing their schema and queries to validate the sqlc-gen-rust implementation.

```
cp -R river/riverdriver/riverpgxv5/migration/main/* sqlc-gen-rust/examples/kitchen-sink-riverqueue/migrations/.
cp -R river/riverdriver/riverpgxv5/internal/dbsqlc/* sqlc-gen-rust/examples/kitchen-sink-riverqueue/queries/.
```

Version 9e57b861174a744a31da83ca757cf3ab232e553b