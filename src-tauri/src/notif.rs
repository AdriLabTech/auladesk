//! Registro de notificaciones (la bandeja de la app). La entrega al escritorio la hace `notifier`.
use crate::error::AppResult;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct NotifRow {
    pub id: i64, pub category: String, pub priority: i64, pub title: String, pub body: Option<String>,
    pub ref_kind: Option<String>, pub ref_id: Option<i64>, pub created_at: i64, pub read_at: Option<i64>, pub shown_at: Option<i64>,
}

pub async fn add(pool: &SqlitePool, category: &str, priority: i64, title: &str, body: Option<&str>, ref_kind: Option<&str>, ref_id: Option<i64>) -> AppResult<i64> {
    Ok(sqlx::query_scalar("INSERT INTO notification(category,priority,title,body,ref_kind,ref_id,created_at) VALUES(?,?,?,?,?,?,strftime('%s','now')) RETURNING id")
        .bind(category).bind(priority).bind(title).bind(body).bind(ref_kind).bind(ref_id).fetch_one(pool).await?)
}

pub async fn list(pool: &SqlitePool, limit: i64, unread_only: bool) -> AppResult<Vec<NotifRow>> {
    Ok(sqlx::query_as("SELECT id,category,priority,title,body,ref_kind,ref_id,created_at,read_at,shown_at FROM notification WHERE (?1=0 OR read_at IS NULL) ORDER BY created_at DESC, id DESC LIMIT ?2")
        .bind(unread_only as i64).bind(limit.clamp(1, 500)).fetch_all(pool).await?)
}

pub async fn mark_read(pool: &SqlitePool, id: Option<i64>) -> AppResult<()> {
    sqlx::query("UPDATE notification SET read_at=strftime('%s','now') WHERE read_at IS NULL AND (?1 IS NULL OR id=?1)").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn unread_count(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM notification WHERE read_at IS NULL").fetch_one(pool).await?)
}
