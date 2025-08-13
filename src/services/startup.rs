use crate::api::user::save_pwd_hash;
use crate::db::user::admin_exists;
use crate::models::user::UserDto;
use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tracing_subscriber::EnvFilter;

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

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                // Default: info logs for our crate, warn for others
                .add_directive("my_app=info".parse().unwrap())
                .add_directive("tower_http=info".parse().unwrap()),
        )
        .with_target(false) // Hide module paths if you want cleaner logs
        .with_file(true) // Show file paths in logs
        .with_line_number(true)
        .compact() // Compact mode: shorter logs
        .init();
}
