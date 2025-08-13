use crate::api::user::save_pwd_hash;
use crate::db::user::admin_exists;
use crate::models::user::UserDto;
use anyhow::Result;
use sqlx::sqlite::SqlitePool;

pub async fn ensure_admin_user(db: &SqlitePool) -> Result<()> {
    let admin_exists: i64 = admin_exists(db).await?;

    if admin_exists == 0 {
        let admin = UserDto {
            username: "admin".to_string(),
            password: "admin".to_string(),
            is_admin: true,
        };
        save_pwd_hash(&admin, db).await?;

        println!("Admin user created: username='admin'");
    }

    Ok(())
}
