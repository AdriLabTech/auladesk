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
    /// Las migraciones ya aplicadas en bases de datos reales NO pueden cambiar ni un byte (sqlx compara checksums → VersionMismatch).
    /// Si necesitas cambiar el esquema, añade una migración nueva; nunca edites una existente (ni un comentario).
    #[test]
    fn applied_migrations_are_frozen() {
        use sha2::{Digest, Sha384};
        let frozen = [
            ("0001_init.sql", "8934ABD67C02B318"), ("0002_library.sql", "B7EF539029C6E19A"),
            ("0003_assignments.sql", "61B0907B47F97D9C"), ("0004_calendar_notifications.sql", "0963BEC458883B6E"),
        ];
        for (name, prefix) in frozen {
            let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations").join(name)).unwrap();
            let h: String = Sha384::digest(&bytes).iter().map(|b| format!("{b:02X}")).collect();
            assert!(h.starts_with(prefix), "{name} ha cambiado: rompería las bases de datos existentes");
        }
    }

    /// Herramienta manual: `AULADESK_TEST_DB=/ruta/copia.db cargo test --lib opens_existing -- --ignored` abre una COPIA de una BD real y aplica las migraciones pendientes.
    #[tokio::test]
    #[ignore]
    async fn opens_existing_database_copy() {
        let p = std::env::var("AULADESK_TEST_DB").expect("define AULADESK_TEST_DB");
        let pool = open(std::path::Path::new(&p)).await.expect("abrir y migrar la copia");
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations").fetch_one(&pool).await.unwrap();
        println!("migraciones aplicadas: {n}");
        assert!(n >= 5);
    }

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
