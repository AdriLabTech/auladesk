//! Registro de notificaciones (la bandeja de la app). La entrega al escritorio la hace `notifier`.
use crate::error::AppResult;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct NotifRow {
    pub id: i64, pub category: String, pub priority: i64, pub title: String, pub body: Option<String>,
    pub ref_kind: Option<String>, pub ref_id: Option<i64>, pub created_at: i64, pub read_at: Option<i64>, pub shown_at: Option<i64>,
    /// Origen: `None` = Moodle/local; `familias` para avisos del portal de familias (con su cuenta y alumno).
    pub source: Option<String>, pub account_id: Option<i64>, pub student_id: Option<i64>,
}

pub async fn add(pool: &SqlitePool, category: &str, priority: i64, title: &str, body: Option<&str>, ref_kind: Option<&str>, ref_id: Option<i64>) -> AppResult<i64> {
    let (title, body) = crate::i18n::localize(pool, title, body).await;
    let (title, body) = (title.as_str(), body.as_deref());
    Ok(sqlx::query_scalar("INSERT INTO notification(category,priority,title,body,ref_kind,ref_id,created_at) VALUES(?,?,?,?,?,?,strftime('%s','now')) RETURNING id")
        .bind(category).bind(priority).bind(title).bind(body).bind(ref_kind).bind(ref_id).fetch_one(pool).await?)
}

pub async fn list(pool: &SqlitePool, limit: i64, unread_only: bool) -> AppResult<Vec<NotifRow>> {
    Ok(sqlx::query_as("SELECT id,category,priority,title,body,ref_kind,ref_id,created_at,read_at,shown_at,source,account_id,student_id FROM notification WHERE (?1=0 OR read_at IS NULL) ORDER BY created_at DESC, id DESC LIMIT ?2")
        .bind(unread_only as i64).bind(limit.clamp(1, 500)).fetch_all(pool).await?)
}

pub async fn mark_read(pool: &SqlitePool, id: Option<i64>) -> AppResult<()> {
    sqlx::query("UPDATE notification SET read_at=strftime('%s','now') WHERE read_at IS NULL AND (?1 IS NULL OR id=?1)").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn unread_count(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM notification WHERE read_at IS NULL").fetch_one(pool).await?)
}

/// Notificación con origen (plataforma/cuenta/alumno) y clave de deduplicación: la misma clave nunca crea dos avisos.
/// Devuelve `None` si ya existía.
pub struct SourcedNotif<'a> {
    pub category: &'a str, pub priority: i64, pub title: &'a str, pub body: Option<&'a str>,
    pub source: &'a str, pub account_id: i64, pub student_id: Option<i64>, pub dedupe_key: &'a str,
}

pub async fn add_sourced(pool: &SqlitePool, n: &SourcedNotif<'_>) -> AppResult<Option<i64>> {
    let (title, body) = crate::i18n::localize(pool, n.title, n.body).await;
    Ok(sqlx::query_scalar(
        "INSERT INTO notification(category,priority,title,body,created_at,source,account_id,student_id,dedupe_key) VALUES(?,?,?,?,strftime('%s','now'),?,?,?,?)
         ON CONFLICT DO NOTHING RETURNING id")
        .bind(n.category).bind(n.priority).bind(&title).bind(&body).bind(n.source).bind(n.account_id).bind(n.student_id).bind(n.dedupe_key)
        .fetch_optional(pool).await?)
}
