//! Almacenamiento de archivos: hash SHA-256, escritura atómica sin sobrescritura y detección de duplicados.
use crate::error::{AppError, AppResult};
use crate::paths::safe_join;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn sha256_bytes(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn sha256_file(path: &Path) -> AppResult<String> {
    let data = tokio::fs::read(path).await?;
    Ok(sha256_bytes(&data))
}

/// Guarda `data` en `root/rel`. Si existe, añade sufijo `_vN`: nunca sobrescribe. Devuelve la ruta relativa final.
pub async fn store_new(root: &Path, rel: &Path, data: &[u8]) -> AppResult<PathBuf> {
    let full = safe_join(root, rel)?;
    if let Some(p) = full.parent() { tokio::fs::create_dir_all(p).await?; }
    let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("archivo").to_string();
    let ext = rel.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let dir = rel.parent().unwrap_or(Path::new(""));
    for n in 1..1000 {
        let name = if n == 1 { format!("{stem}{ext}") } else { format!("{stem}_v{n}{ext}") };
        let cand_rel = dir.join(name);
        let cand = safe_join(root, &cand_rel)?;
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&cand).await {
            Ok(mut f) => {
                use tokio::io::AsyncWriteExt;
                f.write_all(data).await?;
                f.sync_all().await?;
                return Ok(cand_rel);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::Other("demasiadas versiones del mismo nombre".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_hash() {
        assert_eq!(sha256_bytes(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
    #[tokio::test]
    async fn never_overwrites() {
        let d = tempfile::tempdir().unwrap();
        let a = store_new(d.path(), Path::new("C/T1/enunciado.pdf"), b"uno").await.unwrap();
        let b = store_new(d.path(), Path::new("C/T1/enunciado.pdf"), b"dos").await.unwrap();
        assert_eq!(a, Path::new("C/T1/enunciado.pdf"));
        assert_eq!(b, Path::new("C/T1/enunciado_v2.pdf"));
        assert_eq!(std::fs::read(d.path().join(&a)).unwrap(), b"uno");
    }
    #[tokio::test]
    async fn rejects_traversal() {
        let d = tempfile::tempdir().unwrap();
        assert!(store_new(d.path(), Path::new("../fuera.txt"), b"x").await.is_err());
    }
}
