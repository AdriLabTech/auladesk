# Arquitectura
Tauri 2.12 (Rust) + Svelte 5/TS + SQLite cifrada con SQLCipher (sqlx 0.8). Linux/Wayland/Hyprland; WebKitGTK 4.1. Windows/macOS compilan pero no se han ejecutado.

```
src/                         UI Svelte → invoke()
  App.svelte                 barra lateral por GRUPOS (core · Moodle · Familias), contexto activo en la cabecera
  lib/sections.ts            secciones con su grupo (Ctrl+1…9 conservan su orden)
  lib/familias.svelte.ts     estado de Familias: cuenta, alumnado y alumno ACTIVO
  views/                     Moodle (Courses, Tasks, Library…) · Familias (Familias*.svelte)
src-tauri/src/
  commands.rs                frontera IPC de Moodle (lista blanca de ajustes)
  db.rs                      pool, migraciones (con copia previa en exports/), cifrado, backup
  accounts.rs                linked_account + student; aislamiento por alumno; desvincular = purgar la cuenta
  integration.rs             trait Integration (estado · sync · disconnect) + registro: Moodle, Familias
  moodle/                    adaptador, mapper, sesión y ventana de Moodle (webview oculto)
  familias/                  integración del portal de familias (independiente de Moodle)
    window.rs                webview propio y oculto (almacén y llavero propios), navegación solo *.juntadeandalucia.es
    mapper.rs                contrato de datos f1: validación, limpieza de texto, hash de contenido
    sync.rs                  persistencia idempotente por alumno + avisos deduplicados (primera carga silenciosa)
    recipes.rs               recetas de navegación (JSON, lista cerrada de pasos)  ·  selectors/familias_recipes.json
    commands.rs              ingest_familias (único comando remoto), orquestación, límites de frecuencia, consultas por alumno
    actions.rs               acciones de escritura con confirmación explícita y registro
  session_store.rs           cookies en el llavero, una entrada por plataforma
  calendar.rs · notif.rs · reminders.rs · notifier.rs    calendario y avisos COMPARTIDOS (origen por fila)
  sync.rs · downloads.rs · library.rs · submissions.rs … específicos de Moodle
selectors/                   bridge.js (Moodle) · familias.js (Familias) · familias_recipes.json
migrations/                  0001–0006 (Moodle, Drive) · 0007 cuentas · 0008 calendario multi-origen · 0009 avisos con origen · 0010 datos de Familias · 0011 acciones
```

## Principios
- **Aislamiento entre plataformas.** Cada una tiene su webview, su almacén de cookies, su entrada de llavero y su estado de sesión (`AppState`). Un fallo de Familias no toca tablas ni sesión de Moodle; se sincronizan por separado.
- **Núcleo compartido, no duplicado.** Calendario (`calendar_event.source`), avisos (`notification.source`, `dedupe_key`), BD, copia de seguridad, temas y paleta son los de siempre; Familias solo añade filas con su origen.
- **Nunca se mezclan alumnos.** Todo dato de Familias lleva `student_id`; cada comando recibe el id explícito y `accounts::check_student` lo valida contra la cuenta.
- **Datos reales o nada.** Una extracción que no cumple el contrato se rechaza entera; sin sesión la UI lo dice y enseña la fecha de los últimos datos. Los extractores no se inventan: sin reconocimiento del portal no hay datos.
- **Superficie remota mínima.** Los webviews de Moodle y Familias solo pueden invocar `ingest_page` / `ingest_familias` (capabilities por origen); ninguna orden de la página se ejecuta.
- **Escritura = confirmación explícita + registro.** Ver `familias/actions.rs`.
- Historial de entregas inmutable (triggers SQL). Datos en `~/.local/share/moodledesk` (override `AULADESK_DATA_DIR`; nombre heredado, no se cambia). Secretos en el llavero, nunca en BD ni logs; las rutas del portal con token se enmascaran en logs.

## Migraciones
Aditivas y versionadas. Antes de aplicar pendientes en una BD existente se guarda `exports/pre-migracion-<de>-a-<a>-<fecha>.db` (si la copia falla, no se migra). `0008` reconstruye `calendar_event` (el `CHECK` de `source` no se puede alterar en SQLite): como sqlx migra dentro de una transacción y ahí `PRAGMA foreign_keys` no actúa, los avisos (`reminder`) se copian a una tabla auxiliar y se restituyen; hay test que lo comprueba y falla si se quita la restitución. Una migración ya publicada no se edita (test de checksums).

## Segundo plano
Servicio `systemd --user` opcional que lee la BD y lanza `notify-send`; la app no necesita estar abierta. Autosync de Moodle (`autosync.rs`) y de Familias (`familias::commands::run_loop`, máximo una vez por hora).
