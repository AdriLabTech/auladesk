use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("base de datos: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migración: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("ruta no permitida: {0}")]
    UnsafePath(String),
    #[error("no implementado todavía: {0}")]
    Unsupported(&'static str),
    #[error("{0}")]
    Other(String),
}

/// Los comandos devuelven el texto al frontend; el detalle técnico va a los logs.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
