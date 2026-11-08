# Arquitectura
Tauri 2.12 (Rust) + Svelte 5/TS + SQLite (sqlx 0.8). Linux/Wayland/Hyprland; WebKitGTK 4.1.

```
src/                 UI Svelte (sidebar, topbar, vistas) → invoke()
src-tauri/src/
  commands.rs        frontera IPC (lista blanca de ajustes)
  db.rs              pool, migraciones, backup (VACUUM INTO), ajustes
  paths.rs           XDG, saneado y safe_join anti-traversal
  files.rs           SHA-256, escritura create_new (nunca sobrescribe)
  moodle/            trait MoodleAdapter + web.rs (Fase 2) + session.rs + models.rs
  sync.rs            persistencia de extracciones (soft-delete, idempotente, rechaza vacíos)
  downloads.rs       integridad, hash, dedupe, versiones, colocación por asignatura/tema
  library.rs         descargas orquestadas (WebView) + gestión de archivos locales + portadas
  devctl.rs          canal SOLO desarrollo (AULADESK_DEV_CTL=1)
  (previstos) calendar, notifications, submissions
```
Principios: el dominio habla con `MoodleAdapter` y modelos internos; la UI nunca toca HTML de Moodle; persistencia local primero; historial de entregas inmutable (triggers SQL). Datos en `~/.local/share/moodledesk` (override `AULADESK_DATA_DIR`). Secretos: libsecret (`keyring`), nunca en BD/logs.
Segundo plano (Fase 5): servicio `systemd --user` que lee la BD y lanza `notify-send`/D-Bus; la app no necesita estar abierta. Sin tocar la config de Hyprland.
