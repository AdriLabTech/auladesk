//! Copia a una carpeta elegida por el usuario: la de Google Drive para escritorio, OneDrive, Dropbox, Nextcloud, Syncthing…
//! Sin cuentas, sin claves ni permisos: AulaDesk solo copia ficheros y el programa de sincronización de esa nube hace el resto.
//! Estructura: `<carpeta>/AulaDesk/<carpetas de la biblioteca>/<archivo>`. Nunca sobrescribe ni borra: mismo contenido (SHA-256) = se omite;
//! contenido distinto con el mismo nombre = sufijo `_vN`.
use crate::error::{AppError, AppResult};
use crate::paths::safe_join;
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const SUBDIR: &str = "AulaDesk";
const KEY_DIR: &str = "backup_folder";
const KEY_SUBMISSIONS: &str = "backup_folder_submissions";
const MAX_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, PartialEq, Serialize)]
pub enum CopyStatus { Copied, Skipped }

/// Copia `src` a `base/AulaDesk/<rel>` sin sobrescribir. `sha` es el hash registrado del contenido de `src`.
pub async fn copy_into(base: &Path, rel: &str, src: &Path, sha: &str) -> AppResult<CopyStatus> {
    let rel = Path::new(rel);
    let target_root = base.join(SUBDIR);
    let first = safe_join(&target_root, rel)?;
    if let Some(p) = first.parent() { tokio::fs::create_dir_all(p).await?; }
    let meta = tokio::fs::metadata(src).await.map_err(|_| AppError::Other("el archivo ya no está en disco".into()))?;
    if meta.len() > MAX_BYTES { return Err(AppError::Other(format!("pesa más de {} MB", MAX_BYTES / 1024 / 1024))); }
    let data = tokio::fs::read(src).await?;
    if crate::files::sha256_bytes(&data) != sha { return Err(AppError::Other("el contenido en disco no coincide con su hash registrado".into())); }
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("archivo").to_string();
    let ext = rel.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let dir = rel.parent().unwrap_or(Path::new(""));
    for n in 1..1000 {
        let name = if n == 1 { format!("{stem}{ext}") } else { format!("{stem}_v{n}{ext}") };
        let cand = safe_join(&target_root, &dir.join(name))?;
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&cand).await {
            Ok(mut f) => {
                use tokio::io::AsyncWriteExt;
                f.write_all(&data).await?;
                f.sync_all().await?;
                return Ok(CopyStatus::Copied);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if crate::files::sha256_file(&cand).await.ok().as_deref() == Some(sha) { return Ok(CopyStatus::Skipped); }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::Other("demasiadas versiones del mismo nombre".into()))
}

/// Comprueba que la carpeta sirve: existe, se puede escribir y no es la propia carpeta de datos de AulaDesk (evita copiarse a sí misma).
pub async fn validate_folder(dir: &Path, data_dir: &Path) -> AppResult<PathBuf> {
    let canon = tokio::fs::canonicalize(dir).await.map_err(|_| AppError::Other("esa carpeta no existe".into()))?;
    if !canon.is_dir() { return Err(AppError::Other("eso no es una carpeta".into())); }
    if let Ok(d) = std::fs::canonicalize(data_dir) { if canon.starts_with(&d) { return Err(AppError::Other("elige una carpeta fuera de la carpeta de datos de AulaDesk".into())); } }
    let probe = canon.join(format!(".auladesk-prueba-{}", uuid::Uuid::new_v4().simple()));
    tokio::fs::write(&probe, b"ok").await.map_err(|e| AppError::Other(format!("no se puede escribir en esa carpeta: {e}")))?;
    let _ = tokio::fs::remove_file(&probe).await;
    Ok(canon)
}

#[derive(Serialize)]
pub struct FolderStatus { pub path: Option<String>, pub available: bool, pub submissions: bool }

#[derive(Serialize)]
pub struct CopyOutcome { pub file_id: i64, pub name: String, pub status: String, pub detail: Option<String> }

async fn configured_dir(st: &AppState) -> Option<PathBuf> {
    crate::db::get_setting(&st.pool, KEY_DIR).await.ok().flatten().filter(|s| !s.is_empty()).map(PathBuf::from)
}

#[tauri::command]
pub async fn folder_backup_status(st: tauri::State<'_, AppState>) -> Result<FolderStatus, String> {
    let dir = configured_dir(&st).await;
    let submissions = crate::db::get_setting(&st.pool, KEY_SUBMISSIONS).await.ok().flatten().as_deref() == Some("1");
    Ok(FolderStatus { available: dir.as_ref().is_some_and(|d| d.is_dir()), path: dir.map(|d| d.display().to_string()), submissions })
}

/// `path = None` quita la carpeta. Los archivos ya copiados no se tocan.
#[tauri::command]
pub async fn folder_backup_set(st: tauri::State<'_, AppState>, path: Option<String>) -> Result<(), String> {
    match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => {
            let canon = validate_folder(Path::new(&p), &st.data_dir).await.map_err(|e| e.to_string())?;
            crate::db::set_setting(&st.pool, KEY_DIR, &canon.display().to_string()).await.map_err(|e| e.to_string())
        }
        None => crate::db::set_setting(&st.pool, KEY_DIR, "").await.map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn folder_backup_submissions_set(st: tauri::State<'_, AppState>, on: bool) -> Result<(), String> {
    crate::db::set_setting(&st.pool, KEY_SUBMISSIONS, if on { "1" } else { "0" }).await.map_err(|e| e.to_string())
}

async fn copy_one(st: &AppState, base: &Path, file_id: i64) -> CopyOutcome {
    let row: Option<(String, String, String)> = sqlx::query_as("SELECT original_name, rel_path, sha256 FROM local_file WHERE id=?").bind(file_id).fetch_optional(&st.pool).await.ok().flatten();
    let Some((name, rel, sha)) = row else { return CopyOutcome { file_id, name: String::new(), status: "error".into(), detail: Some("archivo desconocido".into()) } };
    match copy_into(base, &rel, &st.data_dir.join("files").join(&rel), &sha).await {
        Ok(CopyStatus::Copied) => CopyOutcome { file_id, name, status: "copied".into(), detail: None },
        Ok(CopyStatus::Skipped) => CopyOutcome { file_id, name, status: "skipped".into(), detail: None },
        Err(e) => CopyOutcome { file_id, name, status: "error".into(), detail: Some(e.to_string()) },
    }
}

/// Copia a la carpeta elegida los archivos de la biblioteca (de una asignatura o de todas). Un fallo no detiene el resto.
#[tauri::command]
pub async fn folder_backup_run(st: tauri::State<'_, AppState>, course_id: Option<i64>) -> Result<Vec<CopyOutcome>, String> {
    let base = configured_dir(&st).await.ok_or("Elige primero una carpeta de copia.")?;
    if !base.is_dir() { return Err("La carpeta de copia no está disponible (¿unidad desconectada?).".into()); }
    let ids: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT l.id FROM local_file l LEFT JOIN file_association fa ON fa.file_id=l.id
        WHERE l.sync_state IN ('synced','personal','outdated') AND (?1 IS NULL OR fa.course_id=?1) ORDER BY l.id LIMIT 2000")
        .bind(course_id).fetch_all(&st.pool).await.map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids { out.push(copy_one(&st, &base, id).await); }
    Ok(out)
}

/// Copia de una entrega ya confirmada (ajuste `backup_folder_submissions`). Los errores solo se registran.
pub async fn backup_confirmed_submission(app: &AppHandle, file_id: i64) {
    let st = app.state::<AppState>();
    if crate::db::get_setting(&st.pool, KEY_SUBMISSIONS).await.ok().flatten().as_deref() != Some("1") { return; }
    let Some(base) = configured_dir(&st).await else { return };
    let r = copy_one(&st, &base, file_id).await;
    match r.status.as_str() {
        "copied" => { let _ = crate::notif::add(&st.pool, "entrega", 0, "Entrega copiada a tu carpeta de copia", Some(&r.name), Some("local_file"), Some(file_id)).await; }
        "error" => tracing::warn!("copia de la entrega a la carpeta: {}", r.detail.unwrap_or_default()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::sha256_bytes;

    #[tokio::test]
    async fn copies_once_then_skips_and_never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let (base, src) = (d.path().join("nube"), d.path().join("a.pdf"));
        std::fs::create_dir(&base).unwrap();
        std::fs::write(&src, b"uno").unwrap();
        let rel = "Interfaces/Tema_01/Recursos/a.pdf";
        assert_eq!(copy_into(&base, rel, &src, &sha256_bytes(b"uno")).await.unwrap(), CopyStatus::Copied);
        assert_eq!(copy_into(&base, rel, &src, &sha256_bytes(b"uno")).await.unwrap(), CopyStatus::Skipped);
        // contenido nuevo con el mismo nombre → _v2; el primero queda intacto
        std::fs::write(&src, b"dos").unwrap();
        assert_eq!(copy_into(&base, rel, &src, &sha256_bytes(b"dos")).await.unwrap(), CopyStatus::Copied);
        assert_eq!(copy_into(&base, rel, &src, &sha256_bytes(b"dos")).await.unwrap(), CopyStatus::Skipped);
        let dir = base.join("AulaDesk/Interfaces/Tema_01/Recursos");
        assert_eq!(std::fs::read(dir.join("a.pdf")).unwrap(), b"uno");
        assert_eq!(std::fs::read(dir.join("a_v2.pdf")).unwrap(), b"dos");
    }

    #[tokio::test]
    async fn rejects_traversal_and_hash_mismatch() {
        let d = tempfile::tempdir().unwrap();
        let (base, src) = (d.path().join("nube"), d.path().join("a.txt"));
        std::fs::create_dir(&base).unwrap();
        std::fs::write(&src, b"x").unwrap();
        assert!(copy_into(&base, "../fuera.txt", &src, &sha256_bytes(b"x")).await.is_err());
        assert!(copy_into(&base, "ok/a.txt", &src, "0000").await.is_err(), "hash distinto = archivo cambiado");
        assert!(!base.join("AulaDesk/ok/a.txt").exists());
    }

    #[tokio::test]
    async fn folder_must_exist_be_writable_and_not_the_data_dir() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("datos");
        std::fs::create_dir_all(data.join("files")).unwrap();
        assert!(validate_folder(&d.path().join("no-existe"), &data).await.is_err());
        assert!(validate_folder(&data.join("files"), &data).await.is_err(), "dentro de la carpeta de datos");
        let ok = d.path().join("nube");
        std::fs::create_dir(&ok).unwrap();
        assert!(validate_folder(&ok, &data).await.is_ok());
        assert!(std::fs::read_dir(&ok).unwrap().next().is_none(), "no deja el fichero de prueba");
    }
}
