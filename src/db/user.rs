use crate::models::user::User;
use sqlx::{Pool, Result, Sqlite};

pub async fn create_user(
    db: &Pool<Sqlite>,
    username: &str,
    is_admin: &bool,
    password_hash: &str,
    salt: &str,
) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, is_admin, password_hash, salt)
        VALUES ($1, $2, $3, $4)
        RETURNING id, is_admin, username, password_hash, salt
        "#,
    )
    .bind(username)
    .bind(is_admin)
    .bind(password_hash)
    .bind(salt)
    .fetch_one(db)
    .await?;

    Ok(user)
}

pub async fn get_user_by_username(db: &Pool<Sqlite>, username: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, username, is_admin, password_hash, salt
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(db)
    .await?;

    Ok(user)
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
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
        .fetch_one(db)
        .await?;

    Ok(count)
}

// pub async fn delete_user(db: &Pool<Sqlite>, user_id: i64) -> Result<()> {
//     sqlx::query(
//         r#"
//         DELETE FROM users WHERE id = $1
//         "#,
//     )
//     .bind(user_id)
//     .execute(db)
//     .await?;

//     Ok(())
// }
