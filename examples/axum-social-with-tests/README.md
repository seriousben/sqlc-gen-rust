Modified from https://github.com/launchbadge/sqlx/tree/028084bce3a741e995c3e6c559c6dbb27a62534d/examples/postgres/axum-social-with-tests

This example demonstrates how to write integration tests for an API build with [Axum] and SQLx using `#[sqlx::test]`.

See also: https://github.com/tokio-rs/axum/blob/main/examples/testing

# Warning

For the sake of brevity, this project omits numerous critical security precautions. You can use it as a starting point,
but deploy to production at your own risk!

[Axum]: https://github.com/tokio-rs/axum