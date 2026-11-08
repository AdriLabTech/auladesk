//! MoodleWebAdapter: se implementa en la Fase 2 sobre el WebView autenticado.
use super::{models::*, MoodleAdapter};
use crate::error::{AppError, AppResult};
use async_trait::async_trait;

pub struct MoodleWebAdapter;

#[async_trait]
impl MoodleAdapter for MoodleWebAdapter {
    fn name(&self) -> &'static str { "web" }
    async fn list_courses(&self) -> AppResult<Vec<RemoteCourse>> { Err(AppError::Unsupported("extracción web (Fase 2)")) }
    async fn course_contents(&self, _: i64) -> AppResult<Vec<RemoteSection>> { Err(AppError::Unsupported("extracción web (Fase 2)")) }
}
