-- :name CreatePost :exec
with inserted_post as (
    insert into post(user_id, content)
    values ($1, $2)
    returning post_id, user_id, content, created_at
)
select post_id, username, content, created_at
from inserted_post
inner join "user" using (user_id);

-- :name GetPosts :many
select post_id, username, content, created_at
from post
inner join "user" using (user_id)
order by created_at desc;

-- name: CreateComment :exec
with inserted_comment as (
    insert into comment(user_id, post_id, content)
    values ($1, $2, $3)
    returning comment_id, user_id, content, created_at
)
select comment_id, username, content, created_at
from inserted_comment
inner join "user" using (user_id);

-- name: GetComments :many
select comment_id, username, content, created_at
from comment
inner join "user" using (user_id)
where post_id = $1
order by created_at;

-- name: CreateUser :exec
insert into "user"(username, password_hash)
values ($1, $2);

-- name: GetUserByUsername :one
select user_id, password_hash from "user" where username = $1;
