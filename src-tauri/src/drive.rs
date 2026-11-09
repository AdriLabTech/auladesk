//! Vinculación con Google Drive (OAuth 2.0 de aplicación de escritorio: navegador del sistema + redirección a 127.0.0.1 + PKCE).
//! Alcance `drive.file`: la app solo ve lo que ella misma sube, dentro de la carpeta «AulaDesk». No lee el resto del Drive.
//! El token de refresco y el secreto del cliente viven en el llavero del sistema, nunca en la BD, los logs ni el frontend.
//! El ID de cliente lo crea el usuario en Google Cloud (tipo «Aplicación de escritorio»): ver docs/DRIVE.md.
use crate::state::AppState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const KEYRING_SERVICE: &str = "auladesk-drive";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
pub const ROOT_FOLDER: &str = "AulaDesk";
const UPLOAD_MAX: u64 = 200 * 1024 * 1024;
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

/// Direcciones de Google. Se pasan como valor para poder probar contra un servidor local sin tocar Google.
#[derive(Clone, Debug)]
pub struct Endpoints { pub auth: String, pub token: String, pub revoke: String, pub api: String, pub upload: String }
impl Default for Endpoints {
    fn default() -> Self {
        Self { auth: "https://accounts.google.com/o/oauth2/v2/auth".into(), token: "https://oauth2.googleapis.com/token".into(), revoke: "https://oauth2.googleapis.com/revoke".into(),
            api: "https://www.googleapis.com/drive/v3".into(), upload: "https://www.googleapis.com/upload/drive/v3".into() }
    }
}

// ───────────── Llavero ─────────────
fn entry(user: &str) -> Result<keyring::Entry, String> { keyring::Entry::new(KEYRING_SERVICE, user).map_err(|e| e.to_string()) }
fn secret_get(user: &str) -> Option<String> { entry(user).ok()?.get_password().ok().filter(|s| !s.is_empty()) }
fn secret_set(user: &str, v: &str) -> Result<(), String> { entry(user)?.set_password(v).map_err(|e| e.to_string()) }
fn secret_del(user: &str) { if let Ok(e) = entry(user) { let _ = e.delete_credential(); } }

// ───────────── PKCE y URL de autorización (puro, con tests) ─────────────
pub fn random_token() -> String { format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()) }
pub fn pkce_challenge(verifier: &str) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) }

fn enc(s: &str) -> String { percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string() }

pub fn auth_url(ep: &Endpoints, client_id: &str, redirect: &str, state: &str, challenge: &str) -> String {
    format!("{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        ep.auth, enc(client_id), enc(redirect), enc(SCOPE), enc(state), enc(challenge))
}

/// Extrae `code` y `state` de la primera línea de la petición HTTP de la redirección («GET /?code=…&state=… HTTP/1.1»).
pub fn parse_callback(request_line: &str) -> Result<(String, String), String> {
    let target = request_line.split_whitespace().nth(1).ok_or("petición vacía")?;
    let query = target.split_once('?').map(|x| x.1).ok_or("la redirección no trae parámetros")?;
    let (mut code, mut state, mut error) = (None, None, None);
    for kv in query.split('&') {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        let v = percent_encoding::percent_decode_str(&v.replace('+', " ")).decode_utf8_lossy().to_string();
        match k { "code" => code = Some(v), "state" => state = Some(v), "error" => error = Some(v), _ => {} }
    }
    if let Some(e) = error { return Err(format!("Google devolvió «{e}» (¿cancelaste el permiso?).")); }
    Ok((code.ok_or("falta el código de autorización")?, state.ok_or("falta el estado")?))
}

/// Ruta de carpetas en Drive para un archivo de la biblioteca: «AulaDesk/<carpetas de rel_path>» (sin el nombre de archivo).
pub fn folder_segments(rel_path: &str) -> Vec<String> {
    let mut v = vec![ROOT_FOLDER.to_string()];
    let parts: Vec<&str> = rel_path.split('/').filter(|p| !p.is_empty() && *p != "." && *p != "..").collect();
    if parts.len() > 1 { v.extend(parts[..parts.len() - 1].iter().map(|s| s.to_string())); }
    v
}

fn q_escape(s: &str) -> String { s.replace('\\', "\\\\").replace('\'', "\\'") }

// ───────────── Tokens ─────────────
#[derive(Deserialize)]
struct TokenResp { access_token: String, expires_in: Option<i64>, refresh_token: Option<String> }

static ACCESS: Mutex<Option<(String, i64)>> = Mutex::new(None);

/// Cliente de Google del propio proyecto AulaDesk, fijado al compilar (`AULADESK_GOOGLE_CLIENT_ID` / `AULADESK_GOOGLE_CLIENT_SECRET`).
/// En aplicaciones de escritorio Google no trata ese «secreto» como confidencial (con PKCE solo identifica a la app), por eso puede ir en el binario.
/// Con él, el usuario solo pulsa «Conectar con Google»; sin él, hay que poner el cliente propio (modo avanzado).
fn bundled_client() -> Option<(String, String)> {
    let (id, secret) = (option_env!("AULADESK_GOOGLE_CLIENT_ID")?, option_env!("AULADESK_GOOGLE_CLIENT_SECRET")?);
    (!id.is_empty() && !secret.is_empty()).then(|| (id.to_string(), secret.to_string()))
}

/// El cliente propio del usuario (ajuste + llavero) tiene prioridad sobre el integrado.
fn client_pair(client_id: Option<String>) -> Result<(String, String), String> {
    if let (Some(id), Some(secret)) = (client_id.filter(|s| !s.is_empty()), secret_get("client-secret")) { return Ok((id, secret)); }
    bundled_client().ok_or_else(|| "Falta el ID de cliente de Google (Configuración de Drive).".to_string())
}

async fn token_post(http: &reqwest::Client, ep: &Endpoints, form: &[(&str, &str)]) -> Result<TokenResp, String> {
    let r = http.post(&ep.token).form(form).send().await.map_err(|e| format!("No se pudo contactar con Google: {e}"))?;
    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().await.unwrap_or_default();
        let reason = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_else(|| status.to_string());
        return Err(if reason == "invalid_grant" { "Google ya no acepta la vinculación (¿la revocaste?). Vuelve a conectar Drive.".into() } else { format!("Google rechazó la autorización: {reason}") });
    }
    r.json::<TokenResp>().await.map_err(|e| format!("Respuesta de Google ilegible: {e}"))
}

async fn access_token(http: &reqwest::Client, ep: &Endpoints, client_id: Option<String>) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    if let Some((t, exp)) = ACCESS.lock().unwrap().clone() { if exp > now + 60 { return Ok(t); } }
    let (id, secret) = client_pair(client_id)?;
    let refresh = secret_get("refresh-token").ok_or("Drive no está conectado.")?;
    let t = token_post(http, ep, &[("client_id", &id), ("client_secret", &secret), ("refresh_token", &refresh), ("grant_type", "refresh_token")]).await?;
    *ACCESS.lock().unwrap() = Some((t.access_token.clone(), now + t.expires_in.unwrap_or(3000)));
    Ok(t.access_token)
}

// ───────────── Conexión ─────────────
const OK_PAGE: &str = "<!doctype html><meta charset=utf-8><title>AulaDesk</title><body style=\"font:16px system-ui;background:#111;color:#eee;display:grid;place-items:center;height:100vh;margin:0\"><div><h2>Drive conectado</h2><p>Ya puedes cerrar esta pestaña y volver a AulaDesk.</p></div>";
const KO_PAGE: &str = "<!doctype html><meta charset=utf-8><title>AulaDesk</title><body style=\"font:16px system-ui;background:#111;color:#eee;display:grid;place-items:center;height:100vh;margin:0\"><div><h2>No se pudo conectar</h2><p>Vuelve a AulaDesk para ver el motivo.</p></div>";

/// Espera la redirección de Google en el puerto local. Valida `state` (anti-CSRF) y responde una página sencilla al navegador.
async fn wait_callback(listener: tokio::net::TcpListener, expected_state: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let work = async {
        loop {
            let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 4096];
            let n = tokio::time::timeout(std::time::Duration::from_secs(5), sock.read(&mut buf)).await.ok().and_then(|r| r.ok()).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            let line = head.lines().next().unwrap_or("");
            if !line.contains("code=") && !line.contains("error=") {   // p. ej. /favicon.ico
                let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                continue;
            }
            let res = parse_callback(line).and_then(|(code, state)| if state == expected_state { Ok(code) } else { Err("El estado de la respuesta no coincide: se descarta por seguridad.".to_string()) });
            let page = if res.is_ok() { OK_PAGE } else { KO_PAGE };
            let _ = sock.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes()).await;
            return res;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(180), work).await.map_err(|_| "Tiempo agotado: no se completó la autorización en el navegador.".to_string())?
}

/// Flujo completo. `open_url` abre el navegador del sistema (inyectable para probar).
pub async fn connect(pool: &SqlitePool, ep: &Endpoints, open_url: impl FnOnce(&str) -> Result<(), String>) -> Result<String, String> {
    let http = reqwest::Client::new();
    let client_id = crate::db::get_setting(pool, "drive_client_id").await.ok().flatten();
    let (id, secret) = client_pair(client_id)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr().map_err(|e| e.to_string())?.port());
    let (verifier, state) = (random_token(), random_token());
    open_url(&auth_url(ep, &id, &redirect, &state, &pkce_challenge(&verifier)))?;
    let code = wait_callback(listener, &state).await?;
    let t = token_post(&http, ep, &[("client_id", &id), ("client_secret", &secret), ("code", &code), ("code_verifier", &verifier), ("redirect_uri", &redirect), ("grant_type", "authorization_code")]).await?;
    let refresh = t.refresh_token.ok_or("Google no devolvió token de refresco. Revoca el acceso en tu cuenta de Google y repite la conexión.")?;
    secret_set("refresh-token", &refresh)?;
    *ACCESS.lock().unwrap() = Some((t.access_token.clone(), chrono::Utc::now().timestamp() + t.expires_in.unwrap_or(3000)));
    let email = about_email(&http, ep, &t.access_token).await.unwrap_or_default();
    crate::db::set_setting(pool, "drive_account", &email).await.map_err(|e| e.to_string())?;
    Ok(email)
}

async fn about_email(http: &reqwest::Client, ep: &Endpoints, token: &str) -> Option<String> {
    let v: serde_json::Value = http.get(format!("{}/about?fields=user(emailAddress)", ep.api)).bearer_auth(token).send().await.ok()?.json().await.ok()?;
    v["user"]["emailAddress"].as_str().map(String::from)
}

pub async fn disconnect(pool: &SqlitePool, ep: &Endpoints) {
    if let Some(rt) = secret_get("refresh-token") { let _ = reqwest::Client::new().post(&ep.revoke).form(&[("token", rt.as_str())]).send().await; }
    secret_del("refresh-token");
    *ACCESS.lock().unwrap() = None;
    let _ = sqlx::query("DELETE FROM drive_folder").execute(pool).await;
    let _ = crate::db::set_setting(pool, "drive_account", "").await;
}

// ───────────── Carpetas y subida ─────────────
async fn api_err(r: reqwest::Response) -> String {
    let s = r.status();
    let body = r.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or_default();
    format!("Drive respondió {s}{}", if msg.is_empty() { String::new() } else { format!(": {msg}") })
}

async fn find_or_create_folder(http: &reqwest::Client, ep: &Endpoints, token: &str, name: &str, parent: Option<&str>) -> Result<String, String> {
    let q = format!("name='{}' and mimeType='{FOLDER_MIME}' and trashed=false and '{}' in parents", q_escape(name), parent.unwrap_or("root"));
    let r = http.get(format!("{}/files", ep.api)).bearer_auth(token).query(&[("q", q.as_str()), ("fields", "files(id)"), ("pageSize", "1")]).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() { return Err(api_err(r).await); }
    let v: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
    if let Some(id) = v["files"][0]["id"].as_str() { return Ok(id.to_string()); }
    let mut meta = serde_json::json!({ "name": name, "mimeType": FOLDER_MIME });
    if let Some(p) = parent { meta["parents"] = serde_json::json!([p]); }
    let r = http.post(format!("{}/files?fields=id", ep.api)).bearer_auth(token).json(&meta).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() { return Err(api_err(r).await); }
    let v: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
    v["id"].as_str().map(String::from).ok_or_else(|| "Drive no devolvió el id de la carpeta".into())
}

/// Crea (o reutiliza, con caché en BD) la cadena de carpetas y devuelve el id de la última.
async fn ensure_path(pool: &SqlitePool, http: &reqwest::Client, ep: &Endpoints, token: &str, segs: &[String]) -> Result<String, String> {
    let mut path = String::new();
    let mut parent: Option<String> = None;
    for s in segs {
        path = if path.is_empty() { s.clone() } else { format!("{path}/{s}") };
        let cached: Option<String> = sqlx::query_scalar("SELECT drive_id FROM drive_folder WHERE path=?").bind(&path).fetch_optional(pool).await.map_err(|e| e.to_string())?;
        let id = match cached { Some(id) => id, None => {
            let id = find_or_create_folder(http, ep, token, s, parent.as_deref()).await?;
            sqlx::query("INSERT OR REPLACE INTO drive_folder(path,drive_id) VALUES(?,?)").bind(&path).bind(&id).execute(pool).await.map_err(|e| e.to_string())?;
            id
        } };
        parent = Some(id);
    }
    parent.ok_or_else(|| "ruta vacía".into())
}

async fn upload_bytes(http: &reqwest::Client, ep: &Endpoints, token: &str, parent: &str, name: &str, bytes: Vec<u8>) -> Result<(String, Option<String>), String> {
    let meta = serde_json::json!({ "name": name, "parents": [parent] });
    let init = http.post(format!("{}/files?uploadType=resumable&fields=id,webViewLink", ep.upload)).bearer_auth(token)
        .header("X-Upload-Content-Length", bytes.len()).json(&meta).send().await.map_err(|e| e.to_string())?;
    if !init.status().is_success() { return Err(api_err(init).await); }
    let loc = init.headers().get("location").and_then(|h| h.to_str().ok()).map(String::from).ok_or("Drive no devolvió la dirección de subida")?;
    let put = http.put(loc).bearer_auth(token).body(bytes).send().await.map_err(|e| e.to_string())?;
    if !put.status().is_success() { return Err(api_err(put).await); }
    let v: serde_json::Value = put.json().await.map_err(|e| e.to_string())?;
    Ok((v["id"].as_str().ok_or("Drive no devolvió el id del archivo")?.to_string(), v["webViewLink"].as_str().map(String::from)))
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct UploadOutcome { pub file_id: i64, pub name: String, pub status: String, pub drive_url: Option<String>, pub detail: Option<String> }

/// Sube un archivo de la biblioteca. Si ese mismo contenido ya está subido, no repite (`skipped`). Nunca borra ni sobrescribe nada en Drive.
pub async fn upload_local_file(pool: &SqlitePool, files_root: &Path, ep: &Endpoints, file_id: i64) -> UploadOutcome {
    let http = reqwest::Client::new();
    let client_id = crate::db::get_setting(pool, "drive_client_id").await.ok().flatten();
    match access_token(&http, ep, client_id).await {
        Ok(token) => upload_with_token(pool, files_root, ep, &token, file_id).await,
        Err(e) => UploadOutcome { file_id, name: String::new(), status: "error".into(), drive_url: None, detail: Some(e) },
    }
}

async fn upload_with_token(pool: &SqlitePool, files_root: &Path, ep: &Endpoints, token: &str, file_id: i64) -> UploadOutcome {
    let row: Option<(String, String, String)> = sqlx::query_as("SELECT original_name, rel_path, sha256 FROM local_file WHERE id=?").bind(file_id).fetch_optional(pool).await.ok().flatten();
    let Some((name, rel, sha)) = row else { return UploadOutcome { file_id, name: String::new(), status: "error".into(), drive_url: None, detail: Some("archivo desconocido".into()) } };
    let fail = |d: String| UploadOutcome { file_id, name: name.clone(), status: "error".into(), drive_url: None, detail: Some(d) };
    if let Ok(Some(url)) = sqlx::query_scalar::<_, Option<String>>("SELECT drive_url FROM drive_item WHERE local_file_id=? AND sha256=?").bind(file_id).bind(&sha).fetch_optional(pool).await {
        return UploadOutcome { file_id, name, status: "skipped".into(), drive_url: url, detail: None };
    }
    let path = match crate::paths::safe_join(files_root, Path::new(&rel)) { Ok(p) => p, Err(e) => return fail(e.to_string()) };
    match tokio::fs::metadata(&path).await { Ok(m) if m.len() > UPLOAD_MAX => return fail(format!("pesa más de {} MB", UPLOAD_MAX / 1024 / 1024)), Ok(_) => {}, Err(_) => return fail("el archivo ya no está en disco".into()) }
    let bytes = match tokio::fs::read(&path).await { Ok(b) => b, Err(e) => return fail(e.to_string()) };
    if crate::files::sha256_bytes(&bytes) != sha { return fail("el contenido en disco no coincide con su hash registrado".into()); }
    let http = reqwest::Client::new();
    let segs = folder_segments(&rel);
    let mut attempt = 0;
    let (drive_id, url) = loop {
        attempt += 1;
        let res = async { let parent = ensure_path(pool, &http, ep, token, &segs).await?; upload_bytes(&http, ep, token, &parent, &name, bytes.clone()).await }.await;
        match res {
            Ok(x) => break x,
            // La carpeta cacheada pudo borrarse en Drive: se olvida la caché y se reintenta una vez.
            Err(e) if attempt == 1 && e.contains("404") => { let _ = sqlx::query("DELETE FROM drive_folder").execute(pool).await; }
            Err(e) => return fail(e),
        }
    };
    let _ = sqlx::query("INSERT OR REPLACE INTO drive_item(local_file_id,sha256,drive_file_id,drive_url,uploaded_at) VALUES(?,?,?,?,strftime('%s','now'))").bind(file_id).bind(&sha).bind(&drive_id).bind(&url).execute(pool).await;
    UploadOutcome { file_id, name, status: "uploaded".into(), drive_url: url, detail: None }
}

// ───────────── Comandos ─────────────
#[derive(Serialize)]
pub struct DriveStatus { pub bundled: bool, pub configured: bool, pub connected: bool, pub account: Option<String>, pub client_id: Option<String>, pub backup_submissions: bool, pub uploaded: i64 }

#[tauri::command]
pub async fn drive_status(st: tauri::State<'_, AppState>) -> Result<DriveStatus, String> {
    let client_id = crate::db::get_setting(&st.pool, "drive_client_id").await.ok().flatten().filter(|s| !s.is_empty());
    let (has_secret, has_refresh) = tauri::async_runtime::spawn_blocking(|| (secret_get("client-secret").is_some(), secret_get("refresh-token").is_some())).await.map_err(|e| e.to_string())?;
    let account = crate::db::get_setting(&st.pool, "drive_account").await.ok().flatten().filter(|s| !s.is_empty());
    let backup = crate::db::get_setting(&st.pool, "drive_backup_submissions").await.ok().flatten().as_deref() == Some("1");
    let uploaded = sqlx::query_scalar("SELECT count(*) FROM drive_item").fetch_one(&st.pool).await.unwrap_or(0);
    Ok(DriveStatus { bundled: bundled_client().is_some(), configured: (client_id.is_some() && has_secret) || bundled_client().is_some(), connected: has_refresh, account, client_id, backup_submissions: backup, uploaded })
}

/// Guarda el ID de cliente (ajuste) y el secreto (llavero). El secreto nunca vuelve al frontend.
#[tauri::command]
pub async fn drive_set_client(st: tauri::State<'_, AppState>, client_id: String, client_secret: String) -> Result<(), String> {
    let id = client_id.trim();
    if !id.ends_with(".apps.googleusercontent.com") { return Err("El ID de cliente debe terminar en «.apps.googleusercontent.com».".into()); }
    let secret = client_secret.trim().to_string();
    if secret.len() < 8 { return Err("El secreto del cliente no parece válido.".into()); }
    crate::db::set_setting(&st.pool, "drive_client_id", id).await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || { secret_del("refresh-token"); secret_set("client-secret", &secret) }).await.map_err(|e| e.to_string())??; // un token de otro cliente no sirve: hay que reconectar
    *ACCESS.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn drive_connect(app: AppHandle) -> Result<String, String> {
    let st = app.state::<AppState>();
    let app2 = app.clone();
    let email = connect(&st.pool, &Endpoints::default(), move |url| tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| format!("No se pudo abrir el navegador: {e}"))).await?;
    let _ = app2.emit("drive-status", "connected");
    Ok(email)
}

#[tauri::command]
pub async fn drive_disconnect(app: AppHandle, forget_client: Option<bool>) -> Result<(), String> {
    let st = app.state::<AppState>();
    disconnect(&st.pool, &Endpoints::default()).await;
    if forget_client.unwrap_or(false) { secret_del("client-secret"); secret_del("refresh-token"); let _ = crate::db::set_setting(&st.pool, "drive_client_id", "").await; }
    let _ = app.emit("drive-status", "disconnected");
    Ok(())
}

fn emit_progress(app: &AppHandle, done: usize, total: usize, name: &str) { let _ = app.emit("drive-progress", serde_json::json!({ "done": done, "total": total, "name": name })); }

/// Sube varios archivos de la biblioteca, uno a uno. Devuelve el resultado de cada uno (un fallo no detiene el resto, salvo falta de conexión).
pub async fn upload_many(app: &AppHandle, ids: Vec<i64>) -> Vec<UploadOutcome> {
    let st = app.state::<AppState>();
    let root = st.data_dir.join("files");
    let ep = Endpoints::default();
    let total = ids.len();
    let mut out = Vec::with_capacity(total);
    for (i, id) in ids.into_iter().enumerate() {
        let r = upload_local_file(&st.pool, &root, &ep, id).await;
        emit_progress(app, i + 1, total, &r.name);
        let stop = r.status == "error" && r.detail.as_deref().is_some_and(|d| d.contains("no está conectado") || d.contains("Vuelve a conectar") || d.contains("Falta el"));
        out.push(r);
        if stop { break; }
    }
    out
}

#[tauri::command]
pub async fn drive_upload_files(app: AppHandle, ids: Vec<i64>) -> Result<Vec<UploadOutcome>, String> {
    if ids.len() > 500 { return Err("Demasiados archivos de una vez (máximo 500).".into()); }
    Ok(upload_many(&app, ids).await)
}

/// `course_id` = esa asignatura; sin él, toda la biblioteca. Solo archivos con contenido aún no subido.
#[tauri::command]
pub async fn drive_upload_pending(app: AppHandle, course_id: Option<i64>) -> Result<Vec<UploadOutcome>, String> {
    let st = app.state::<AppState>();
    let ids: Vec<i64> = sqlx::query_scalar("SELECT DISTINCT l.id FROM local_file l LEFT JOIN file_association fa ON fa.file_id=l.id
        WHERE l.sync_state IN ('synced','personal','outdated') AND (?1 IS NULL OR fa.course_id=?1)
          AND NOT EXISTS (SELECT 1 FROM drive_item d WHERE d.local_file_id=l.id AND d.sha256=l.sha256) ORDER BY l.id LIMIT 500")
        .bind(course_id).fetch_all(&st.pool).await.map_err(|e| e.to_string())?;
    Ok(upload_many(&app, ids).await)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DriveFileState { pub local_file_id: i64, pub drive_url: Option<String>, pub uploaded_at: i64 }

/// Qué archivos (con su contenido actual) ya están en Drive, para mostrar la insignia en la biblioteca.
#[tauri::command]
pub async fn drive_file_states(st: tauri::State<'_, AppState>) -> Result<Vec<DriveFileState>, String> {
    sqlx::query_as("SELECT d.local_file_id, d.drive_url, d.uploaded_at FROM drive_item d JOIN local_file l ON l.id=d.local_file_id AND l.sha256=d.sha256")
        .fetch_all(&st.pool).await.map_err(|e| e.to_string())
}

/// Copia de seguridad en Drive de una entrega ya confirmada (ajuste `drive_backup_submissions`). Los errores solo se registran.
pub async fn backup_confirmed_submission(app: &AppHandle, file_id: i64) {
    let st = app.state::<AppState>();
    if crate::db::get_setting(&st.pool, "drive_backup_submissions").await.ok().flatten().as_deref() != Some("1") { return; }
    if !tauri::async_runtime::spawn_blocking(|| secret_get("refresh-token").is_some()).await.unwrap_or(false) { return; }
    let r = upload_local_file(&st.pool, &st.data_dir.join("files"), &Endpoints::default(), file_id).await;
    match r.status.as_str() {
        "uploaded" => { let _ = crate::notif::add(&st.pool, "entrega", 0, "Entrega guardada en Drive", Some(&r.name), Some("local_file"), Some(file_id)).await; }
        "error" => tracing::warn!("copia en Drive de la entrega: {}", r.detail.unwrap_or_default()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_client_is_absent_unless_set_at_build_time() {
        // En los tests no se define AULADESK_GOOGLE_CLIENT_ID: sin cliente integrado y sin cliente propio, el error debe decirlo.
        if bundled_client().is_none() { assert!(client_pair(None).is_err()); }
    }

    #[test]
    fn pkce_matches_rfc7636_example() {
        // Vector de prueba del RFC 7636, apéndice B.
        assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
    #[test]
    fn tokens_are_long_and_unique() { let (a, b) = (random_token(), random_token()); assert_eq!(a.len(), 64); assert_ne!(a, b); }
    #[test]
    fn auth_url_has_required_params() {
        let u = auth_url(&Endpoints::default(), "id.apps.googleusercontent.com", "http://127.0.0.1:4242", "st", "ch");
        for p in ["response_type=code", "code_challenge_method=S256", "access_type=offline", "state=st", "code_challenge=ch", "drive%2Efile", "redirect_uri=http%3A%2F%2F127%2E0%2E0%2E1%3A4242"] { assert!(u.contains(p), "falta {p} en {u}"); }
        assert!(!u.contains("drive.readonly") && !u.contains("auth/drive "), "solo debe pedir drive.file");
    }
    #[test]
    fn callback_parsing() {
        assert_eq!(parse_callback("GET /?code=4%2F0AbC&state=xyz HTTP/1.1").unwrap(), ("4/0AbC".into(), "xyz".into()));
        assert!(parse_callback("GET /?error=access_denied&state=x HTTP/1.1").unwrap_err().contains("access_denied"));
        assert!(parse_callback("GET / HTTP/1.1").is_err());
        assert!(parse_callback("").is_err());
    }
    #[test]
    fn folder_segments_mirror_library() {
        assert_eq!(folder_segments("Programación/Tema_01/Recursos/apuntes.pdf"), vec!["AulaDesk", "Programación", "Tema_01", "Recursos"]);
        assert_eq!(folder_segments("suelto.pdf"), vec!["AulaDesk"]);
        assert_eq!(folder_segments("../../etc/x/../a.txt"), vec!["AulaDesk", "etc", "x"], "nunca segmentos de escape");
    }
    #[test]
    fn query_escaping() { assert_eq!(q_escape("Pepe's \\ dir"), "Pepe\\'s \\\\ dir"); }

    // ── Servidor de Google simulado (solo local): comprueba el protocolo, NO sustituye una prueba contra Google real ──
    use std::sync::{Arc, Mutex as StdMutex};
    async fn mock_google(log: Arc<StdMutex<Vec<String>>>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let b2 = base.clone();
        tokio::spawn(async move { let mut n = 0; loop {
            let (mut s, _) = l.accept().await.unwrap();
            let (log, base) = (log.clone(), b2.clone());
            n += 1; let n = n;
            tokio::spawn(async move {
                let mut buf = Vec::new(); let mut tmp = [0u8; 8192];
                // lee cabeceras y cuerpo según Content-Length
                loop { let k = s.read(&mut tmp).await.unwrap_or(0); if k == 0 { break; } buf.extend_from_slice(&tmp[..k]);
                    if let Some(h) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..h]).to_lowercase();
                        let cl = head.lines().find_map(|l| l.strip_prefix("content-length: ")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                        if buf.len() >= h + 4 + cl { break; } } }
                let head = String::from_utf8_lossy(&buf).to_string();
                let line = head.lines().next().unwrap_or("").to_string();
                log.lock().unwrap().push(line.clone());
                let (status, extra, body) = if line.starts_with("GET /files?") { ("200 OK", String::new(), "{\"files\":[]}".to_string()) }
                    else if line.starts_with("POST /files?fields=id ") { ("200 OK", String::new(), format!("{{\"id\":\"folder{n}\"}}")) }
                    else if line.starts_with("POST /upload/files?uploadType=resumable") { ("200 OK", format!("Location: {base}/session/1\r\n"), "{}".to_string()) }
                    else if line.starts_with("PUT /session/1") { ("200 OK", String::new(), "{\"id\":\"file1\",\"webViewLink\":\"https://drive.example/file1\"}".to_string()) }
                    else { ("404 Not Found", String::new(), "{}".to_string()) };
                let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
            });
        } });
        base
    }

    #[tokio::test]
    async fn uploads_into_mirrored_folders_once() {
        let d = tempfile::tempdir().unwrap();
        let pool = crate::db::open(&d.path().join("t.db")).await.unwrap();
        let root = d.path().join("files");
        tokio::fs::create_dir_all(root.join("Asig/Tema_01/Recursos")).await.unwrap();
        let data = b"%PDF-1 contenido de prueba";
        tokio::fs::write(root.join("Asig/Tema_01/Recursos/a.pdf"), data).await.unwrap();
        let sha = crate::files::sha256_bytes(data);
        let fid: i64 = sqlx::query_scalar("INSERT INTO local_file(uuid,original_name,local_name,rel_path,extension,size_bytes,sha256,downloaded_at) VALUES('u','a.pdf','a.pdf','Asig/Tema_01/Recursos/a.pdf','pdf',?,?,1) RETURNING id")
            .bind(data.len() as i64).bind(&sha).fetch_one(&pool).await.unwrap();
        let log = Arc::new(StdMutex::new(vec![]));
        let base = mock_google(log.clone()).await;
        let ep = Endpoints { auth: String::new(), token: String::new(), revoke: String::new(), api: base.clone(), upload: format!("{base}/upload") };

        let r = upload_with_token(&pool, &root, &ep, "tok", fid).await;
        assert_eq!((r.status.as_str(), r.drive_url.as_deref()), ("uploaded", Some("https://drive.example/file1")), "{r:?}");
        let creates = log.lock().unwrap().iter().filter(|l| l.starts_with("POST /files?fields=id ")).count();
        assert_eq!(creates, 4, "AulaDesk/Asig/Tema_01/Recursos");
        // mismo contenido: no se vuelve a subir
        let before = log.lock().unwrap().len();
        let r2 = upload_with_token(&pool, &root, &ep, "tok", fid).await;
        assert_eq!(r2.status, "skipped");
        assert_eq!(log.lock().unwrap().len(), before, "sin peticiones nuevas");
        // contenido modificado en disco sin actualizar el hash registrado: se rechaza
        tokio::fs::write(root.join("Asig/Tema_01/Recursos/a.pdf"), b"otro").await.unwrap();
        sqlx::query("DELETE FROM drive_item").execute(&pool).await.unwrap();
        assert_eq!(upload_with_token(&pool, &root, &ep, "tok", fid).await.status, "error");
    }
}
