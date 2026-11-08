//! Biblioteca documental: integridad, deduplicación por SHA-256, versiones y colocación por asignatura/tema.
use crate::error::{AppError, AppResult};
use crate::paths::{safe_join, sanitize_component};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Stored { pub sha256: String, pub file_id: i64, pub rel_path: String, pub original_name: String, pub duplicate: bool, pub new_version: bool, pub size: i64 }

pub async fn hash_and_head(path: &Path) -> AppResult<(String, i64, Vec<u8>)> {
    let mut f = tokio::fs::File::open(path).await?;
    let (mut h, mut buf, mut size) = (Sha256::new(), vec![0u8; 64 * 1024], 0i64);
    let mut head = Vec::new();
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 { break; }
        if head.len() < 512 { head.extend_from_slice(&buf[..n.min(512 - head.len())]); }
        h.update(&buf[..n]);
        size += n as i64;
    }
    Ok((h.finalize().iter().map(|b| format!("{b:02x}")).collect(), size, head))
}

/// Comprueba que el contenido es coherente con la extensión. Un HTML con extensión de documento suele ser una página de login.
pub fn verify_content(ext: &str, head: &[u8]) -> Result<(), String> {
    let ext = ext.to_ascii_lowercase();
    if head.is_empty() { return Err("archivo vacío".into()); }
    let starts = |p: &[u8]| head.starts_with(p);
    let ok = match ext.as_str() {
        "pdf" => starts(b"%PDF"),
        "zip" | "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" | "jar" | "apk" => starts(b"PK"),
        "png" => starts(b"\x89PNG"),
        "jpg" | "jpeg" => starts(&[0xFF, 0xD8]),
        _ => true,
    };
    if !ok { return Err(format!("el contenido no es un .{ext} válido (¿descarga corrupta o sesión caducada?)")); }
    if !matches!(ext.as_str(), "html" | "htm" | "xhtml" | "svg" | "xml") {
        let s = String::from_utf8_lossy(&head[..head.len().min(200)]).trim_start().to_ascii_lowercase();
        if s.starts_with("<!doctype html") || s.starts_with("<html") {
            return Err("la descarga es una página web, no el archivo (¿sesión caducada?)".into());
        }
    }
    Ok(())
}

pub fn sanitize_filename(name: &str) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()) => (s, Some(e.to_ascii_lowercase())),
        _ => (name, None),
    };
    let stem = sanitize_component(stem);
    match ext { Some(e) => format!("{stem}.{e}"), None => stem }
}

pub fn section_dir(position: i64) -> String { if position == 0 { "General".into() } else { format!("Tema_{position:02}") } }
pub fn kind_dir(modname: &str) -> &'static str { if matches!(modname, "assign" | "quiz") { "Actividades" } else { "Recursos" } }

pub fn name_from_url(url: &str) -> String {
    let last = url.split('?').next().unwrap_or("").rsplit('/').next().unwrap_or("archivo");
    use unicode_normalization::UnicodeNormalization;
    percent_encoding::percent_decode_str(last).decode_utf8_lossy().nfc().collect()
}

/// Mueve `tmp` a `root/rel` sin sobrescribir (sufijo _vN). Devuelve la ruta relativa final.
async fn place(root: &Path, rel: &Path, tmp: &Path) -> AppResult<PathBuf> {
    let full = safe_join(root, rel)?;
    if let Some(p) = full.parent() { tokio::fs::create_dir_all(p).await?; }
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("archivo").to_string();
    let ext = rel.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let dir = rel.parent().unwrap_or(Path::new(""));
    for n in 1..1000 {
        let cand_rel = dir.join(if n == 1 { format!("{stem}{ext}") } else { format!("{stem}_v{n}{ext}") });
        let cand = safe_join(root, &cand_rel)?;
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&cand).await {
            Ok(_) => { tokio::fs::rename(tmp, &cand).await?; return Ok(cand_rel); }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::Other("demasiadas versiones del mismo nombre".into()))
}

/// Registra un archivo descargado (en `tmp`) y lo coloca en `files_root`. Si `activity_id` es None va a «Sin_clasificar».
pub async fn ingest(pool: &SqlitePool, files_root: &Path, tmp: &Path, source_url: &str, suggested: &str, activity_id: Option<i64>) -> AppResult<Stored> {
    // El nombre sugerido por el navegador no es fiable (añade « (1)» si ya existe en la carpeta de descargas del usuario):
    // se prefiere el de la URL de Moodle y el sugerido queda de reserva.
    let from_url = name_from_url(source_url);
    let original = if from_url.contains('.') || suggested.is_empty() { from_url } else { suggested.to_string() };
    let clean = sanitize_filename(&original);
    let ext = Path::new(&clean).extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let (sha, size, head) = hash_and_head(tmp).await?;
    if let Err(e) = verify_content(&ext, &head) { let _ = tokio::fs::remove_file(tmp).await; return Err(AppError::Other(e)); }

    let loc: Option<(i64, i64, i64, String, i64, String)> = match activity_id {
        Some(a) => sqlx::query_as("SELECT c.id, s.id, a.id, c.folder_name, s.position, a.modname FROM activity a JOIN course c ON c.id=a.course_id JOIN course_section s ON s.id=a.section_id WHERE a.id=?")
            .bind(a).fetch_optional(pool).await?,
        None => None,
    };
    let role = match loc.as_ref().map(|l| l.5.as_str()) { Some("assign") => "statement", Some(_) => "resource", None => "personal" };

    // ¿Ya existe el mismo contenido? No se duplica el archivo; solo se asocia.
    let existing: Option<(i64, String)> = sqlx::query_as("SELECT id, rel_path FROM local_file WHERE sha256=? ORDER BY id LIMIT 1").bind(&sha).fetch_optional(pool).await?;
    if let Some((fid, rel)) = existing {
        let _ = tokio::fs::remove_file(tmp).await;
        let mut final_rel = rel.clone();
        if let Some((c, s, a, folder, pos, modname)) = &loc {
            associate(pool, fid, *c, *s, *a, role).await?;
            // Archivo descargado antes «a ciegas» (sin asignatura): se adopta y se coloca donde corresponde.
            if rel.starts_with("Sin_clasificar/") {
                let name = Path::new(&rel).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| clean.clone());
                let target = PathBuf::from(folder).join(section_dir(*pos)).join(kind_dir(modname)).join(name);
                let moved = place(files_root, &target, &files_root.join(&rel)).await?;
                let (moved_s, lname) = (moved.to_string_lossy().to_string(), moved.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string());
                sqlx::query("INSERT INTO course_resource(activity_id,filename,size_bytes,source_url) VALUES(?,?,?,?) ON CONFLICT(activity_id,source_url) DO UPDATE SET size_bytes=excluded.size_bytes")
                    .bind(a).bind(&original).bind(size).bind(source_url).execute(pool).await?;
                let rid: i64 = sqlx::query_scalar("SELECT id FROM course_resource WHERE activity_id=? AND source_url=?").bind(a).bind(source_url).fetch_one(pool).await?;
                sqlx::query("UPDATE local_file SET rel_path=?, local_name=?, sync_state='synced', resource_id=? WHERE id=?").bind(&moved_s).bind(&lname).bind(rid).bind(fid).execute(pool).await?;
                final_rel = moved_s;
            }
        }
        return Ok(Stored { sha256: sha.clone(), file_id: fid, rel_path: final_rel, original_name: original, duplicate: true, new_version: false, size });
    }

    let rel_dir: PathBuf = match &loc {
        Some((_, _, _, folder, pos, modname)) => PathBuf::from(folder).join(section_dir(*pos)).join(kind_dir(modname)),
        None => PathBuf::from("Sin_clasificar"),
    };
    let rel = place(files_root, &rel_dir.join(&clean), tmp).await?;
    let rel_s = rel.to_string_lossy().to_string();
    let local_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or(&clean).to_string();

    let res: AppResult<(i64, bool)> = async {
        let mut tx = pool.begin().await?;
        let mut resource_id: Option<i64> = None;
        let mut new_version = false;
        if let Some((_, _, a, ..)) = &loc {
            sqlx::query("INSERT INTO course_resource(activity_id,filename,size_bytes,source_url) VALUES(?,?,?,?) ON CONFLICT(activity_id,source_url) DO UPDATE SET filename=excluded.filename, size_bytes=excluded.size_bytes")
                .bind(a).bind(&original).bind(size).bind(source_url).execute(&mut *tx).await?;
            let rid: i64 = sqlx::query_scalar("SELECT id FROM course_resource WHERE activity_id=? AND source_url=?").bind(a).bind(source_url).fetch_one(&mut *tx).await?;
            resource_id = Some(rid);
            let n = sqlx::query("UPDATE local_file SET sync_state='outdated' WHERE resource_id=? AND sync_state='synced'").bind(rid).execute(&mut *tx).await?.rows_affected();
            new_version = n > 0;
        }
        let fid: i64 = sqlx::query_scalar(
            "INSERT INTO local_file(uuid,original_name,local_name,rel_path,extension,size_bytes,sha256,source_url,downloaded_at,sync_state,resource_id)
             VALUES(?,?,?,?,?,?,?,?,strftime('%s','now'),?,?) RETURNING id")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&original).bind(&local_name).bind(&rel_s).bind(&ext).bind(size).bind(&sha).bind(source_url)
            .bind(if loc.is_some() { "synced" } else { "personal" }).bind(resource_id).fetch_one(&mut *tx).await?;
        if let Some((c, s, a, ..)) = &loc {
            sqlx::query("INSERT INTO file_association(file_id,course_id,section_id,activity_id,role,created_at) VALUES(?,?,?,?,?,strftime('%s','now'))")
                .bind(fid).bind(c).bind(s).bind(a).bind(role).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok((fid, new_version))
    }.await;
    match res {
        Ok((file_id, new_version)) => Ok(Stored { sha256: sha.clone(), file_id, rel_path: rel_s, original_name: original, duplicate: false, new_version, size }),
        Err(e) => { let _ = tokio::fs::remove_file(files_root.join(&rel)).await; Err(e) } // sin fila no debe quedar archivo huérfano
    }
}

async fn associate(pool: &SqlitePool, fid: i64, c: i64, s: i64, a: i64, role: &str) -> AppResult<()> {
    let have: i64 = sqlx::query_scalar("SELECT count(*) FROM file_association WHERE file_id=? AND activity_id=?").bind(fid).bind(a).fetch_one(pool).await?;
    if have == 0 {
        sqlx::query("INSERT INTO file_association(file_id,course_id,section_id,activity_id,role,created_at) VALUES(?,?,?,?,?,strftime('%s','now'))")
            .bind(fid).bind(c).bind(s).bind(a).bind(role).execute(pool).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, moodle::models::*, sync};

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    async fn setup() -> (tempfile::TempDir, SqlitePool, i64) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Desarrollo de Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let act = |c, m: &str| RemoteActivity { cmid: c, modname: m.into(), name: "x".into(), url: None, due_at: None, files: vec![] };
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 3, name: "T3".into(), activities: vec![act(10, "assign"), act(11, "resource")] }]).await.unwrap();
        let aid: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        (d, p, aid)
    }
    async fn tmpfile(d: &Path, n: &str, data: &[u8]) -> PathBuf { let p = d.join(n); tokio::fs::write(&p, data).await.unwrap(); p }

    #[test]
    fn integrity_checks() {
        assert!(verify_content("pdf", b"%PDF-1.7 ...").is_ok());
        assert!(verify_content("pdf", b"<!DOCTYPE html><html>login").is_err());
        assert!(verify_content("docx", b"PK\x03\x04").is_ok());
        assert!(verify_content("docx", b"%PDF").is_err());
        assert!(verify_content("txt", b"  <html><body>login").is_err());
        assert!(verify_content("html", b"<html>").is_ok());
        assert!(verify_content("pdf", b"").is_err());
    }
    #[test]
    fn names() {
        assert_eq!(sanitize_filename("Introducción a C# y .NET.PDF"), "Introducción_a_C_y_.NET.pdf");
        assert_eq!(sanitize_filename("../../x.pdf"), "x.pdf");
        assert_eq!(name_from_url("https://h/pluginfile.php/1/mod_resource/content/3/Introduccio%CC%81n.pdf?forcedownload=1"), "Introducción.pdf");
    }

    #[tokio::test]
    async fn stores_in_course_section_kind_dir_and_hashes() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let t = tmpfile(d.path(), "a.part", b"%PDF-1.4 hola").await;
        let s = ingest(&p, &root, &t, "https://h/enunciado.pdf?forcedownload=1", "enunciado (1).pdf", Some(aid)).await.unwrap();
        assert_eq!(s.rel_path, "Desarrollo_de_Interfaces/Tema_03/Actividades/enunciado.pdf");
        assert_eq!(std::fs::read(root.join(&s.rel_path)).unwrap(), b"%PDF-1.4 hola");
        let (sha, role): (String, String) = sqlx::query_as("SELECT l.sha256, a.role FROM local_file l JOIN file_association a ON a.file_id=l.id").fetch_one(&p).await.unwrap();
        assert_eq!(sha, crate::files::sha256_bytes(b"%PDF-1.4 hola"));
        assert_eq!(role, "statement");
        assert!(!t.exists(), "el temporal se mueve");
    }

    #[tokio::test]
    async fn same_content_is_not_duplicated_and_new_content_is_new_version() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let url = "https://h/x.pdf";
        let a = ingest(&p, &root, &tmpfile(d.path(), "1", b"%PDF-1").await, url, "x.pdf", Some(aid)).await.unwrap();
        let b = ingest(&p, &root, &tmpfile(d.path(), "2", b"%PDF-1").await, url, "x.pdf", Some(aid)).await.unwrap();
        assert!(b.duplicate && b.file_id == a.file_id);
        let c = ingest(&p, &root, &tmpfile(d.path(), "3", b"%PDF-2 cambiado").await, url, "x.pdf", Some(aid)).await.unwrap();
        assert!(c.new_version && !c.duplicate);
        assert_eq!(c.rel_path, "Desarrollo_de_Interfaces/Tema_03/Actividades/x_v2.pdf", "no sobrescribe la anterior");
        let states: Vec<String> = sqlx::query_scalar("SELECT sync_state FROM local_file ORDER BY id").fetch_all(&p).await.unwrap();
        assert_eq!(states, ["outdated", "synced"]);
        assert!(root.join(&a.rel_path).exists());
    }

    #[tokio::test]
    async fn rejects_login_page_saved_as_pdf_and_leaves_nothing() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let r = ingest(&p, &root, &tmpfile(d.path(), "h", b"<!DOCTYPE html><html>").await, "https://h/x.pdf", "x.pdf", Some(aid)).await;
        assert!(r.is_err());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM local_file").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        assert!(!root.join("Desarrollo_de_Interfaces").exists());
    }

    #[tokio::test]
    async fn blind_download_is_adopted_into_its_activity() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let url = "https://h/mod_resource/content/3/x.pdf";
        let a = ingest(&p, &root, &tmpfile(d.path(), "1", b"%PDF-1").await, url, "x.pdf", None).await.unwrap();
        assert_eq!(a.rel_path, "Sin_clasificar/x.pdf");
        let b = ingest(&p, &root, &tmpfile(d.path(), "2", b"%PDF-1").await, url, "x.pdf", Some(aid)).await.unwrap();
        assert!(b.duplicate && b.file_id == a.file_id);
        assert_eq!(b.rel_path, "Desarrollo_de_Interfaces/Tema_03/Actividades/x.pdf");
        assert!(root.join(&b.rel_path).exists() && !root.join("Sin_clasificar/x.pdf").exists(), "se movió, no se copió");
        let (rel, st, rid): (String, String, Option<i64>) = sqlx::query_as("SELECT rel_path, sync_state, resource_id FROM local_file").fetch_one(&p).await.unwrap();
        assert_eq!((rel.as_str(), st.as_str(), rid.is_some()), (b.rel_path.as_str(), "synced", true));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM file_association WHERE activity_id=?").bind(aid).fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn unclassified_downloads_go_to_sin_clasificar() {
        let (d, p, _) = setup().await;
        let root = d.path().join("files");
        let s = ingest(&p, &root, &tmpfile(d.path(), "z", b"PK\x03\x04zip").await, "https://h/a.zip", "a.zip", None).await.unwrap();
        assert_eq!(s.rel_path, "Sin_clasificar/a.zip");
    }
}
