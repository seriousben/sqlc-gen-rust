use axum::{Extension, Json, Router};

use axum::routing::get;

use serde::{Deserialize, Serialize};

use crate::http::user::UserAuth;
use sqlx::PgPool;
use validator::Validate;

use crate::db::queries;
use crate::http::Result;

use uuid::Uuid;

mod comment;

pub fn router() -> Router {
    Router::new()
        .route("/v1/post", get(get_posts).post(create_post))
        .merge(comment::router())
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct CreatePostRequest {
    auth: UserAuth,
    #[validate(length(min = 1, max = 1000))]
    content: String,
}

#[serde_with::serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Post {
    post_id: Uuid,
    username: String,
    content: String,
    created_at: String,
}

impl From<queries::CreatePostRow> for Post {
    fn from(row: queries::CreatePostRow) -> Self {
        Post {
            post_id: row.post_id,
            username: row.username,
            content: row.content,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

impl From<queries::GetPostsRow> for Post {
    fn from(row: queries::GetPostsRow) -> Self {
        Post {
            post_id: row.post_id,
            username: row.username,
            content: row.content,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

// #[axum::debug_handler] // very useful!
async fn create_post(
    db: Extension<PgPool>,
    Json(req): Json<CreatePostRequest>,
) -> Result<Json<Post>> {
    req.validate()?;
    let user_id = req.auth.verify(&*db).await?;

    let post = queries::create_post(&*db, user_id, req.content).await?;

    Ok(Json(Post::from(post)))
}

/// Returns posts in descending chronological order.
async fn get_posts(db: Extension<PgPool>) -> Result<Json<Vec<Post>>> {
    // Note: normally you'd want to put a `LIMIT` on this as well,
    // though that would also necessitate implementing pagination.
    let posts = queries::get_posts(&*db).await?;

    let posts_api: Vec<Post> = posts.into_iter().map(Post::from).collect();

    Ok(Json(posts_api))
}
