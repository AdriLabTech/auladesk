//! Contrato mínimo común de las plataformas educativas. Es DELIBERADAMENTE pequeño: estado, sincronizar y desvincular.
//! No hay `authenticate()`: el inicio de sesión es interactivo en el WebView de cada plataforma, y las capacidades propias
//! (entregas de Moodle, alumno activo de Familias…) son comandos de su módulo, no del trait.
//! Cada implementación es independiente: un fallo de una no puede parar a otra (se invocan por separado).
use crate::accounts;
use crate::error::{AppError, AppResult};
use crate::moodle::session::SessionState;
use crate::state::AppState;
use async_trait::async_trait;
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum IntegrationStatus { Connected, Expired, Disconnected, Unavailable { reason: String } }

#[async_trait]
pub trait Integration: Send + Sync {
    fn platform(&self) -> &'static str;
    fn label(&self) -> &'static str;
    async fn status(&self, app: &AppHandle) -> IntegrationStatus;
    /// Lectura idempotente. Devuelve un resumen en texto para la interfaz.
    async fn sync(&self, app: &AppHandle) -> AppResult<String>;
    /// Borra la sesión (cookies y llavero) y, salvo en Moodle, los datos de la cuenta.
    async fn disconnect(&self, app: &AppHandle) -> AppResult<()>;
}

pub fn from_session(s: SessionState) -> IntegrationStatus {
    match s { SessionState::Active => IntegrationStatus::Connected, SessionState::Expired => IntegrationStatus::Expired, _ => IntegrationStatus::Disconnected }
}

/// Envuelve lo que ya existe, sin moverlo.
pub struct MoodleIntegration;
#[async_trait]
impl Integration for MoodleIntegration {
    fn platform(&self) -> &'static str { "moodle" }
    fn label(&self) -> &'static str { "Moodle" }
    async fn status(&self, app: &AppHandle) -> IntegrationStatus { from_session(*app.state::<AppState>().session.lock().unwrap()) }
    async fn sync(&self, app: &AppHandle) -> AppResult<String> {
        let r = crate::commands::run_sync(app.clone()).await.map_err(AppError::Other)?;
        Ok(format!("{} cursos, {} actualizados, {} tareas, {} fallos", r.courses, r.synced, r.assignments, r.failed.len()))
    }
    async fn disconnect(&self, app: &AppHandle) -> AppResult<()> { crate::commands::forget_moodle(app).await }
}

pub struct FamiliasIntegration;
#[async_trait]
impl Integration for FamiliasIntegration {
    fn platform(&self) -> &'static str { "familias" }
    fn label(&self) -> &'static str { "Familias" }
    async fn status(&self, app: &AppHandle) -> IntegrationStatus { from_session(*app.state::<AppState>().familias_session.lock().unwrap()) }
    async fn sync(&self, app: &AppHandle) -> AppResult<String> {
        let r = crate::familias::commands::run_sync(app.clone(), true).await.map_err(AppError::Other)?;
        Ok(format!("{} recetas, {} elementos, {} cambios, {} fallos", r.recipes, r.seen, r.changed, r.failed.len()))
    }
    async fn disconnect(&self, app: &AppHandle) -> AppResult<()> {
        let st = app.state::<AppState>();
        crate::familias::commands::familias_disconnect(app.clone(), st).await
    }
}

pub fn registry() -> Vec<Box<dyn Integration>> { vec![Box::new(MoodleIntegration), Box::new(FamiliasIntegration)] }

pub fn find(platform: &str) -> Option<Box<dyn Integration>> { registry().into_iter().find(|i| i.platform() == platform) }

#[derive(Serialize)]
pub struct IntegrationInfo { pub platform: &'static str, pub label: &'static str, pub status: IntegrationStatus, pub last_ok_at: Option<i64>, pub last_error: Option<String> }

#[tauri::command]
pub async fn integrations_list(app: AppHandle, st: tauri::State<'_, AppState>) -> AppResult<Vec<IntegrationInfo>> {
    let accs = accounts::list(&st.pool).await?;
    let mut out = vec![];
    for i in registry() {
        let a = accs.iter().find(|a| a.platform == i.platform());
        out.push(IntegrationInfo { platform: i.platform(), label: i.label(), status: i.status(&app).await, last_ok_at: a.and_then(|a| a.last_ok_at), last_error: a.and_then(|a| a.last_error.clone()) });
    }
    Ok(out)
}

/// Sincroniza UNA plataforma; las demás no se tocan.
#[tauri::command]
pub async fn integration_sync(app: AppHandle, platform: String) -> AppResult<String> {
    find(&platform).ok_or_else(|| AppError::Other(format!("plataforma desconocida: {platform}")))?.sync(&app).await
}

#[derive(Serialize)]
pub struct PlatformOutcome { pub platform: &'static str, pub label: &'static str, pub ok: bool, pub message: String, pub millis: u128 }

/// Sincroniza Moodle y Familias A LA VEZ. Cada una va en su propio WebView y su propia tarea: no se esperan entre sí, y el fallo de una
/// (sin sesión, estructura cambiada, límite de frecuencia…) no afecta a la otra. 
pub async fn sync_all_with(items: Vec<Box<dyn Integration>>, app: &AppHandle) -> Vec<PlatformOutcome> {
    let futs = items.iter().map(|i| {
        let (platform, label) = (i.platform(), i.label());
        async move {
            let t = std::time::Instant::now();
            let r = i.sync(app).await;
            PlatformOutcome { platform, label, ok: r.is_ok(), message: match r { Ok(m) => m, Err(e) => e.to_string() }, millis: t.elapsed().as_millis() }
        }
    });
    futures_join_all(futs).await
}

/// `join_all` sin dependencias nuevas: todas las futuras avanzan a la vez en la misma tarea (cada una espera sobre su propio WebView).
pub(crate) async fn futures_join_all<T, F: std::future::Future<Output = T>>(futs: impl Iterator<Item = F>) -> Vec<T> {
    let handles: Vec<_> = futs.map(|f| Box::pin(f)).collect();
    let mut out = Vec::with_capacity(handles.len());
    let mut pending = handles;
    // sondeo cooperativo: se avanza cada futura en turnos hasta que todas terminan
    let mut done: Vec<Option<T>> = (0..pending.len()).map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut all = true;
        for (i, f) in pending.iter_mut().enumerate() {
            if done[i].is_none() { match f.as_mut().poll(cx) { std::task::Poll::Ready(v) => done[i] = Some(v), std::task::Poll::Pending => all = false } }
        }
        if all { std::task::Poll::Ready(()) } else { std::task::Poll::Pending }
    }).await;
    for d in done { if let Some(v) = d { out.push(v); } }
    out
}

#[tauri::command]
pub async fn sync_all(app: AppHandle) -> AppResult<Vec<PlatformOutcome>> { Ok(sync_all_with(registry(), &app).await) }

#[cfg(test)]
mod tests {
    use super::*;
    fn outcome(p: &'static str, ok: bool, ms: u64) -> impl std::future::Future<Output = PlatformOutcome> {
        async move { let t = std::time::Instant::now(); tokio::time::sleep(std::time::Duration::from_millis(ms)).await; PlatformOutcome { platform: p, label: p, ok, message: String::new(), millis: t.elapsed().as_millis() } }
    }

    #[tokio::test]
    async fn platforms_sync_at_the_same_time_and_fail_independently() {
        let t = std::time::Instant::now();
        let r = futures_join_all(vec![outcome("moodle", true, 300), outcome("familias", false, 300)].into_iter()).await;
        let total = t.elapsed().as_millis();
        assert!(total < 500, "en paralelo tardan ~300 ms, no ~600 (tardó {total} ms)");
        assert_eq!(r.iter().map(|o| (o.platform, o.ok)).collect::<Vec<_>>(), [("moodle", true), ("familias", false)], "el fallo de una no afecta a la otra y se conserva el orden");
    }

    #[test]
    fn registry_has_two_distinct_platforms_matching_the_db_check() {
        let ids: Vec<_> = registry().iter().map(|i| i.platform()).collect();
        assert_eq!(ids, ["moodle", "familias"]);
        for id in ids { assert!(accounts::PLATFORMS.contains(&id)); }
        assert!(find("otra").is_none());
    }
    #[test]
    fn session_maps_to_status() {
        assert_eq!(from_session(SessionState::Active), IntegrationStatus::Connected);
        assert_eq!(from_session(SessionState::Expired), IntegrationStatus::Expired);
        assert_eq!(from_session(SessionState::LoggedOut), IntegrationStatus::Disconnected);
        assert_eq!(from_session(SessionState::Unknown), IntegrationStatus::Disconnected);
        let j = serde_json::to_value(IntegrationStatus::Unavailable { reason: "x".into() }).unwrap();
        assert_eq!(j["state"], "unavailable");
    }
}
