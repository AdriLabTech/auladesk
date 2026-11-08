//! Rutas XDG y operaciones de ruta seguras.
use crate::error::{AppError, AppResult};
use std::path::{Component, Path, PathBuf};

pub const SUBDIRS: [&str; 5] = ["database", "files", "exports", "logs", "cache"];

pub fn default_data_dir() -> AppResult<PathBuf> {
    // OJO: «moodledesk» es el identificador HEREDADO del proyecto (antes se llamaba así). Se conserva para no perder datos locales ni la sesión del llavero.
    let dirs = directories::ProjectDirs::from("dev", "adrian", "moodledesk")
        .ok_or_else(|| AppError::Other("no se pudo resolver XDG_DATA_HOME".into()))?;
    // Por requisito: ~/.local/share/moodledesk (data_dir ya es .../moodledesk).
    Ok(dirs.data_dir().to_path_buf())
}

pub fn ensure_layout(root: &Path) -> AppResult<()> {
    for d in SUBDIRS {
        std::fs::create_dir_all(root.join(d))?;
    }
    Ok(())
}

/// Convierte un nombre de Moodle en un componente de ruta seguro.
pub fn sanitize_component(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let cleaned: String = name
        .nfc()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == '_');
    let collapsed = trimmed.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_");
    if collapsed.is_empty() { "sin_nombre".into() } else { collapsed.chars().take(120).collect() }
}

/// Une `rel` a `root` rechazando absolutas, `..` y prefijos. No toca el disco.
pub fn safe_join(root: &Path, rel: &Path) -> AppResult<PathBuf> {
    for c in rel.components() {
        match c {
            Component::Normal(_) => {}
            _ => return Err(AppError::UnsafePath(rel.display().to_string())),
        }
    }
    Ok(root.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanitize() {
        assert_eq!(sanitize_component("Desarrollo de Interfaces"), "Desarrollo_de_Interfaces");
        assert_eq!(sanitize_component("../../etc/passwd"), "etc_passwd");
        assert_eq!(sanitize_component("..."), "sin_nombre");
        assert_eq!(sanitize_component("Introduccio\u{301}n"), "Introducción", "NFD → NFC");
    }
    #[test]
    fn join_rejects_traversal() {
        let r = Path::new("/data");
        assert!(safe_join(r, Path::new("a/b.pdf")).is_ok());
        assert!(safe_join(r, Path::new("../x")).is_err());
        assert!(safe_join(r, Path::new("/etc/passwd")).is_err());
    }
}
