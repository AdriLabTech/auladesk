use crate::error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

pub async fn open(db_path: &Path) -> AppResult<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Copia de seguridad consistente (VACUUM INTO falla si el destino existe: nunca sobrescribe).
pub async fn backup(pool: &SqlitePool, dest: &Path) -> AppResult<()> {
    sqlx::query("VACUUM INTO ?").bind(dest.to_string_lossy().to_string()).execute(pool).await?;
    Ok(())
}

pub async fn get_setting(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT value FROM application_settings WHERE key = ?")
        .bind(key).fetch_optional(pool).await?)
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO application_settings(key,value,updated_at) VALUES(?,?,strftime('%s','now'))
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
    ).bind(key).bind(value).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrate_settings_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        assert_eq!(get_setting(&pool, "theme").await.unwrap(), None);
        set_setting(&pool, "theme", "dark").await.unwrap();
        set_setting(&pool, "theme", "light").await.unwrap();
        assert_eq!(get_setting(&pool, "theme").await.unwrap().as_deref(), Some("light"));
        let b = dir.path().join("b.db");
        backup(&pool, &b).await.unwrap();
        assert!(b.exists());
        assert!(backup(&pool, &b).await.is_err(), "no debe sobrescribir");
    }

    #[tokio::test]
    async fn submission_history_is_append_only() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open(&dir.path().join("t.db")).await.unwrap();
        sqlx::query("INSERT INTO course(id,moodle_id,fullname,folder_name) VALUES(1,10,'C','C')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO activity(id,course_id,moodle_cmid,modname,name) VALUES(1,1,5,'assign','T')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO local_file(id,uuid,original_name,local_name,rel_path,size_bytes,sha256,downloaded_at) VALUES(1,'u','a','a','a',1,'h',0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO submission(id,activity_id,created_at) VALUES(1,1,0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO submission_version(submission_id,version,file_id,sha256,state) VALUES(1,1,1,'h','prepared')").execute(&pool).await.unwrap();
        assert!(sqlx::query("DELETE FROM submission_version").execute(&pool).await.is_err());
        assert!(sqlx::query("UPDATE submission_version SET sha256='x'").execute(&pool).await.is_err());
        // el estado sí puede avanzar
        sqlx::query("UPDATE submission_version SET state='sent'").execute(&pool).await.unwrap();
    }
}
