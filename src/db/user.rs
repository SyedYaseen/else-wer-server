use crate::models::user::User;
use sqlx::{Pool, Result, Sqlite};

pub async fn create_user(
    db: &Pool<Sqlite>,
    username: &str,
    is_admin: &bool,
    password_hash: &str,
    salt: &str,
    can_organize: &bool,
) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, is_admin, password_hash, salt, can_organize)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, is_admin, username, password_hash, salt, can_organize
        "#,
    )
    .bind(username)
    .bind(is_admin)
    .bind(password_hash)
    .bind(salt)
    .bind(can_organize)
    .fetch_one(db)
    .await?;

    Ok(user)
}

pub async fn get_user_by_username(db: &Pool<Sqlite>, username: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, is_admin, password_hash, salt, can_organize
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(db)
    .await?;

    Ok(user)
}

pub async fn get_user_by_id(db: &Pool<Sqlite>, id: i64) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, is_admin, password_hash, salt, can_organize
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    Ok(user)
}

pub async fn list_users(db: &Pool<Sqlite>) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, is_admin, password_hash, salt, can_organize
        FROM users
        ORDER BY username
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(users)
}

pub async fn update_user_permissions(
    db: &Pool<Sqlite>,
    user_id: i64,
    is_admin: bool,
    can_organize: bool,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE users
        SET is_admin = $1, can_organize = $2
        WHERE id = $3
        "#,
    )
    .bind(is_admin)
    .bind(can_organize)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn update_user_password(
    db: &Pool<Sqlite>,
    user_id: i64,
    new_hash: &str,
    new_salt: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $1, salt = $2
        WHERE id = $3
        "#,
    )
    .bind(new_hash)
    .bind(new_salt)
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn admin_exists(db: &Pool<Sqlite>) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
        .fetch_one(db)
        .await?;

    Ok(count)
}

pub async fn delete_user(db: &Pool<Sqlite>, user_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM users WHERE id = $1
        "#,
    )
    .bind(user_id)
    .execute(db)
    .await?;

    Ok(())
}
