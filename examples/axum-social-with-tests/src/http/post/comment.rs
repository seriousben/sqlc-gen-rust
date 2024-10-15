use axum::extract::Path;
use axum::{Extension, Json, Router};

use axum::routing::get;

use serde::{Deserialize, Serialize};

use crate::http::user::UserAuth;
use sqlx::PgPool;
use validator::Validate;

use crate::db::queries;
use crate::http::Result;

use uuid::Uuid;

pub fn router() -> Router {
    Router::new().route(
        "/v1/post/:postId/comment",
        get(get_post_comments).post(create_post_comment),
    )
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct CreateCommentRequest {
    auth: UserAuth,
    #[validate(length(min = 1, max = 1000))]
    content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Comment {
    comment_id: Uuid,
    username: String,
    content: String,
    created_at: String,
}

impl From<queries::CreateCommentRow> for Comment {
    fn from(row: queries::CreateCommentRow) -> Self {
        Comment {
            comment_id: row.comment_id,
            username: row.username,
            content: row.content,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

impl From<queries::GetCommentsRow> for Comment {
    fn from(row: queries::GetCommentsRow) -> Self {
        Comment {
            comment_id: row.comment_id,
            username: row.username,
            content: row.content,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

// #[axum::debug_handler] // very useful!
async fn create_post_comment(
    db: Extension<PgPool>,
    Path(post_id): Path<Uuid>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<Comment>> {
    req.validate()?;
    let user_id = req.auth.verify(&*db).await?;

    let comment = queries::create_comment(
        &*db,
        queries::CreateCommentInfo {
            user_id,
            post_id,
            content: req.content,
        },
    )
    .await?;

    let comment_api = Comment::from(comment);

    Ok(Json(comment_api))
}

/// Returns comments in ascending chronological order.
async fn get_post_comments(
    db: Extension<PgPool>,
    Path(post_id): Path<Uuid>,
) -> Result<Json<Vec<Comment>>> {
    // Note: normally you'd want to put a `LIMIT` on this as well,
    // though that would also necessitate implementing pagination.
    let comments = queries::get_comments(&*db, post_id).await?;

    let comments = comments.into_iter().map(Comment::from).collect();

    Ok(Json(comments))
}
