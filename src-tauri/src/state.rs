use crate::moodle::session::SessionState;
use crate::sync::SyncStats;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::{oneshot, watch};

pub struct DlCtx { pub hash_only: bool, pub activity_id: Option<i64>, pub tx: tokio::sync::mpsc::UnboundedSender<Result<crate::downloads::Stored, String>> }
pub type FoundFiles = Result<Vec<(String, String)>, String>;
pub type IngestOutcome = Result<SyncStats, String>;

pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub session: Mutex<SessionState>,
    /// Se incrementa cada vez que termina de cargar una página de Moodle.
    pub page_loads: watch::Sender<u64>,
    /// Resultado de la última extracción solicitada por el orquestador.
    pub pending: Mutex<Option<oneshot::Sender<IngestOutcome>>>,
    pub dl: Mutex<Option<DlCtx>>,
    pub dl_started: std::sync::atomic::AtomicBool,
    pub dl_name: Mutex<Option<String>>,
    pub found_files: Mutex<Option<oneshot::Sender<FoundFiles>>>,
    pub sync_running: std::sync::atomic::AtomicBool,
}
