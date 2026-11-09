use crate::error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

/// Abre una BD SQLite SIN cifrar (tests y copias). La aplicación usa `open_app`.
pub async fn open(db_path: &Path) -> AppResult<SqlitePool> { open_with(db_path, None).await }

/// Abre una BD cifrada con SQLCipher. `key_hex` son 64 hex (256 bits). Con clave incorrecta falla (no abre una BD vacía).
pub async fn open_encrypted(db_path: &Path, key_hex: &str) -> AppResult<SqlitePool> { open_with(db_path, Some(key_hex)).await }

async fn open_with(db_path: &Path, key_hex: Option<&str>) -> AppResult<SqlitePool> {
    let mut opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    if let Some(k) = key_hex { opts = opts.pragma("key", key_pragma(k)); } // sqlx aplica `key` antes que el resto de pragmas
    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn key_pragma(hex: &str) -> String { format!("\"x'{hex}'\"") }

fn valid_key(hex: &str) -> bool { hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) }

/// Clave nueva de 256 bits (dos UUID v4 del generador del sistema: 244 bits aleatorios).
pub fn new_key() -> String { format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()) }

/// ¿El fichero es una BD SQLite sin cifrar? (las cifradas no tienen la cabecera «SQLite format 3»).
pub fn is_plaintext(path: &Path) -> bool {
    use std::io::Read;
    let mut h = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut h)).is_ok() && &h == b"SQLite format 3\0"
}

/// Cifra una BD en claro: exporta a un fichero nuevo, comprueba que las tablas y el nº de filas coinciden y solo entonces
/// lo pone en su lugar. El original NO se borra: queda como `<nombre>.sin-cifrar` (devuelve su ruta) hasta que el usuario lo elimine.
pub async fn encrypt_in_place(path: &Path, key_hex: &str) -> AppResult<std::path::PathBuf> {
    use sqlx::{ConnectOptions, Connection, Row};
    if !valid_key(key_hex) { return Err(crate::error::AppError::Other("clave de BD no válida".into())); }
    let tmp = path.with_extension("db.cifrando");
    let keep = path.with_extension("db.sin-cifrar");
    if keep.exists() { return Err(crate::error::AppError::Other(format!("ya existe {}: retírala antes de cifrar", keep.display()))); }
    let _ = std::fs::remove_file(&tmp);
    let q = |p: &Path| p.to_string_lossy().replace('\'', "''");
    let mut c = SqliteConnectOptions::new().filename(path).foreign_keys(false).create_if_missing(true).connect().await?; // create_if_missing: ATTACH necesita poder crear el fichero cifrado
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&mut c).await?;
    sqlx::query(&format!("ATTACH DATABASE '{}' AS e KEY {}", q(&tmp), key_pragma(key_hex))).execute(&mut c).await?;
    sqlx::query("SELECT sqlcipher_export('e')").fetch_one(&mut c).await?;
    sqlx::query("DETACH DATABASE e").execute(&mut c).await?;
    let tables: Vec<String> = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .fetch_all(&mut c).await?.iter().map(|r| r.get::<String, _>(0)).collect();
    let mut before = Vec::new();
    for t in &tables { before.push(sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM \"{t}\"")).fetch_one(&mut c).await?); }
    c.close().await?;
    // verificación con la clave, sin migrar ni escribir
    let mut v = SqliteConnectOptions::new().filename(&tmp).pragma("key", key_pragma(key_hex)).connect().await?;
    for (t, n) in tables.iter().zip(&before) {
        let after: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM \"{t}\"")).fetch_one(&mut v).await?;
        if after != *n { drop(v); let _ = std::fs::remove_file(&tmp); return Err(crate::error::AppError::Other(format!("cifrado cancelado: la tabla {t} tiene {after} filas y debía tener {n}"))); }
    }
    v.close().await?;
    for ext in ["-wal", "-shm"] { let mut p = path.as_os_str().to_owned(); p.push(ext); let _ = std::fs::remove_file(p); }
    std::fs::rename(path, &keep)?;
    std::fs::rename(&tmp, path)?;
    Ok(keep)
}

const KEY_SERVICE: &str = "moodledesk"; // nombre heredado del servicio de llavero: no se cambia para no perder datos
const KEY_USER: &str = "database-key";

fn key_entry() -> Result<keyring::Entry, String> { keyring::Entry::new(KEY_SERVICE, KEY_USER).map_err(|e| e.to_string()) }

/// Lee la clave del llavero del sistema o, si no hay, crea una y la guarda (y comprueba que se puede volver a leer).
fn load_or_create_key(create: bool) -> Result<Option<String>, String> {
    let e = key_entry()?;
    match e.get_password() {
        Ok(k) if valid_key(&k) => Ok(Some(k)),
        Ok(_) => Err("la clave de la BD en el llavero no es válida".into()),
        Err(keyring::Error::NoEntry) if create => {
            let k = new_key();
            e.set_password(&k).map_err(|e| e.to_string())?;
            if e.get_password().ok().as_deref() != Some(k.as_str()) { return Err("no se pudo comprobar la clave guardada en el llavero".into()); }
            Ok(Some(k))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Apertura de la BD de la aplicación, cifrada con SQLCipher; la clave vive en el llavero del sistema.
/// - BD nueva → se crea cifrada. - BD en claro → se cifra con copia previa (`encrypt_in_place`).
/// - Sin llavero: si la BD está en claro se sigue en claro (con aviso); si ya está cifrada se devuelve error SIN tocar nada.
pub async fn open_app(db_path: &Path) -> AppResult<SqlitePool> {
    use crate::error::AppError;
    let exists = db_path.exists();
    let plain = exists && is_plaintext(db_path);
    if exists && !plain {
        let key = load_or_create_key(false).map_err(AppError::Other)?
            .ok_or_else(|| AppError::Other("la base de datos está cifrada y no hay clave en el llavero del sistema".into()))?;
        return open_encrypted(db_path, &key).await;
    }
    match load_or_create_key(true) {
        Ok(Some(key)) => {
            if plain {
                let keep = encrypt_in_place(db_path, &key).await?;
                tracing::warn!("BD cifrada con SQLCipher. La copia sin cifrar sigue en {}: bórrala cuando compruebes que todo funciona", keep.display());
            }
            open_encrypted(db_path, &key).await
        }
        Ok(None) => unreachable!(),
        Err(e) => {
            tracing::warn!("sin llavero ({e}): la BD sigue SIN cifrar");
            open(db_path).await
        }
    }
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

    const K: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[tokio::test]
    async fn encrypts_existing_database_keeping_data_and_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        let pool = open(&p).await.unwrap();
        set_setting(&pool, "theme_id", "secreto-alumno").await.unwrap();
        pool.close().await;
        assert!(is_plaintext(&p));
        let keep = encrypt_in_place(&p, K).await.unwrap();
        assert!(keep.exists() && is_plaintext(&keep), "el original queda como copia");
        assert!(!is_plaintext(&p), "la BD ya no tiene cabecera SQLite en claro");
        assert!(!std::fs::read(&p).unwrap().windows(14).any(|w| w == b"secreto-alumno"));
        let pool = open_encrypted(&p, K).await.unwrap();
        assert_eq!(get_setting(&pool, "theme_id").await.unwrap().as_deref(), Some("secreto-alumno"));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations").fetch_one(&pool).await.unwrap();
        assert!(n >= 5, "las migraciones siguen registradas");
        pool.close().await;
        assert!(open_encrypted(&p, &"ab".repeat(32)).await.is_err(), "clave incorrecta");
        assert!(open(&p).await.is_err(), "sin clave");
    }

    #[tokio::test]
    async fn encrypted_backup_is_encrypted_and_opens_with_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        let pool = open_encrypted(&p, K).await.unwrap();
        set_setting(&pool, "k", "v").await.unwrap();
        let b = dir.path().join("b.db");
        backup(&pool, &b).await.unwrap();
        assert!(!is_plaintext(&b), "la copia de seguridad de una BD cifrada debe ir cifrada");
        let pb = open_encrypted(&b, K).await.unwrap();
        assert_eq!(get_setting(&pb, "k").await.unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn keys_are_256_bit_hex_and_unique() {
        let (a, b) = (new_key(), new_key());
        assert!(valid_key(&a) && valid_key(&b) && a != b);
        assert!(!valid_key("xyz") && !valid_key(&"g".repeat(64)));
    }

    /// Manual: cifra una COPIA de una BD real. `AULADESK_TEST_DB=/ruta/copia.db cargo test --lib encrypts_real_copy -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn encrypts_real_copy() {
        let p = std::path::PathBuf::from(std::env::var("AULADESK_TEST_DB").expect("define AULADESK_TEST_DB"));
        let keep = encrypt_in_place(&p, K).await.expect("cifrar la copia");
        // Sin migrar: una BD real puede traer migraciones de otra rama (p. ej. la 0006 de Drive) que este código no conoce.
        use sqlx::{ConnectOptions, Connection};
        let mut c = SqliteConnectOptions::new().filename(&p).pragma("key", key_pragma(K)).connect().await.expect("abrir cifrada");
        let t: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='table'").fetch_one(&mut c).await.unwrap();
        let m: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations").fetch_one(&mut c).await.unwrap();
        assert!(!is_plaintext(&p));
        println!("OK: {t} tablas, {m} migraciones registradas; original en {}", keep.display());
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
