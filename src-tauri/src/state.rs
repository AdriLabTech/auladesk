use crate::moodle::session::SessionState;
use crate::sync::SyncStats;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::{oneshot, watch};

pub struct DlCtx { pub hash_only: bool, pub activity_id: Option<i64>, pub tx: tokio::sync::mpsc::UnboundedSender<Result<crate::downloads::Stored, String>> }
pub type FoundFiles = Result<Vec<(String, String)>, String>;
pub type IngestOutcome = Result<SyncStats, String>;
/// Ventanas ocultas extra con las que se leen cursos y tareas en paralelo (`moodle-w0`…).
/// 0 = desactivado. Medido el 2026-11-11 con 3 ventanas: 117 s frente a 104 s en serie (Moodle serializa por sesión y cada carga se ralentiza), sin ganancia.
pub const WORKERS: usize = 0;

pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub session: Mutex<SessionState>,
    /// Se incrementa cada vez que termina de cargar una página de Moodle.
    pub page_loads: watch::Sender<u64>,
    /// Resultado de la última extracción solicitada por el orquestador.
    pub pending: Mutex<Option<oneshot::Sender<IngestOutcome>>>,
    /// Igual que `page_loads`/`pending`, una por ventana de lectura en paralelo.
    pub worker_loads: [watch::Sender<u64>; WORKERS],
    pub worker_pending: [Mutex<Option<oneshot::Sender<IngestOutcome>>>; WORKERS],
    pub dl: Mutex<Option<DlCtx>>,
    pub dl_started: std::sync::atomic::AtomicBool,
    pub dl_name: Mutex<Option<String>>,
    pub found_files: Mutex<Option<oneshot::Sender<FoundFiles>>>,
    /// La próxima sincronización de Moodle relee todas las páginas (sin el refresco por niveles).
    pub sync_full: std::sync::atomic::AtomicBool,
    /// El arranque (startup.rs) se encarga de sincronizar Familias: la sincronización «al entrar» no se lanza también.
    pub startup_claims_familias: std::sync::atomic::AtomicBool,
    pub sync_running: std::sync::atomic::AtomicBool,
    // ── Familias (estado propio: nada de esto toca el de Moodle) ──
    pub familias_session: Mutex<SessionState>,
    pub familias_page_loads: watch::Sender<u64>,
    pub familias_pending: Mutex<Option<oneshot::Sender<IngestOutcome>>>,
    pub familias_running: std::sync::atomic::AtomicBool,
}
