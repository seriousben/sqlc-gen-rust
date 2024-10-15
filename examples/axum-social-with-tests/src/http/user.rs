use axum::http::StatusCode;
use axum::{routing::post, Extension, Json, Router};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use std::time::Duration;

use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::db::queries;
use crate::http::{Error, Result};

pub type UserId = Uuid;

pub fn router() -> Router {
    Router::new().route("/v1/user", post(create_user))
}

static USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[0-9A-Za-z_]+$").unwrap());

// CREATE USER

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserAuth {
    #[validate(length(min = 3, max = 16), regex = "USERNAME_REGEX")]
    username: String,
    #[validate(length(min = 8, max = 32))]
    password: String,
}

// WARNING: this API has none of the checks that a normal user signup flow implements,
// such as email or phone verification.
async fn create_user(db: Extension<PgPool>, Json(req): Json<UserAuth>) -> Result<StatusCode> {
    req.validate()?;

    let UserAuth { username, password } = req;

    // It would be irresponsible to store passwords in plaintext, however.
    let password_hash = crate::password::hash(password).await?;

    queries::create_user(&*db, username, password_hash)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(dbe) if dbe.constraint() == Some("user_username_key") => {
                Error::Conflict("username taken".into())
            }
            _ => e.into(),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

impl UserAuth {
    // NOTE: normally we wouldn't want to verify the username and password every time,
    // but persistent sessions would have complicated the example.
    pub async fn verify<'e>(
        self,
        db: impl sqlx::Executor<'e, Database = sqlx::Postgres>,
    ) -> Result<UserId> {
        self.validate()?;

        let user_res = queries::get_user_by_username(db, self.username).await;

        match user_res {
            Ok(user) => {
                let verified = crate::password::verify(self.password, user.password_hash).await?;

                if verified {
                    return Ok(user.user_id);
                }
            }
            Err(e) => {
                if let sqlx::Error::RowNotFound = e {
                    // no-op
                } else {
                    return Err(e.into());
                }
            }
        }

        // Sleep a random amount of time to avoid leaking existence of a user in timing.
        let sleep_duration =
            rand::thread_rng().gen_range(Duration::from_millis(100)..=Duration::from_millis(500));
        tokio::time::sleep(sleep_duration).await;
        Err(Error::UnprocessableEntity(
            "invalid username/password".into(),
        ))
    }
}
