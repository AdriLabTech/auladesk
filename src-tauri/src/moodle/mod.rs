//! Integración con Moodle desacoplada por adaptadores. El dominio solo conoce estos tipos.
pub mod dates;
pub mod mapper;
pub mod models;
pub mod session;
pub mod web;
pub mod window;

use crate::error::AppResult;
use async_trait::async_trait;
use models::*;

/// Fuente de datos académicos. Implementaciones: `web` (navegación autenticada) y, en el futuro, `api` (token oficial).
#[async_trait]
pub trait MoodleAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list_courses(&self) -> AppResult<Vec<RemoteCourse>>;
    async fn course_contents(&self, course_moodle_id: i64) -> AppResult<Vec<RemoteSection>>;
}
