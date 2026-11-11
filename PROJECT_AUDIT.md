# PROJECT_AUDIT — AulaDesk (repo `moodledesk`)

> **Nota 2026-11-11:** el árbol sucio de Drive que se menciona abajo ya se resolvió (commits `0.2.0`/`0.2.1`); la integración de Familias está implementada en la rama `integracion-familias` (ver `INTEGRATION_DESIGN.md` §0 bis y `CHANGELOG.md`). Este documento se conserva como foto de la Fase 0.

Auditoría de la Fase 0. Fecha: 2026-11-09. **No se ha modificado código.** Solo se añaden `PROJECT_AUDIT.md` e `INTEGRATION_RESEARCH.md`.
Todo lo de abajo sale de leer el código y los docs del repo; lo que no se ha comprobado se marca **(no verificado)**.

## 0. Estado del repositorio
- Ruta: `~/Proyectos/moodledesk` (paquete `auladesk` v0.1.1, remoto `AdriLabTech/auladesk`). El repo, la BD (`moodledesk.db`) y el servicio de llavero conservan el nombre antiguo «moodledesk».
- Rama `main`, último commit `d980957` (Moodle como webview embebido en Linux).
- **Árbol sucio (32 entradas)**: trabajo de Drive y entregas programadas sin commitear (`drive.rs`, `scheduler.rs`, migración `0006`, `DriveCard`, `ScheduleForm`, `PendingPrompt`, permisos autogenerados…). No tocar sin que Adrián lo decida; cualquier rama de trabajo nueva debe partir de aquí sin pisarlo.
- Baseline: `cargo test --lib` → **80 passed, 0 failed, 1 ignored** (ejecutado hoy). CI en `.github/workflows/{ci,release}.yml` (contenido no revisado en detalle).

## 1. Arquitectura actual
**Tauri 2.12 (Rust) + Svelte 5 / TypeScript (Vite 8) + SQLite (sqlx 0.8)**, Linux/Wayland como plataforma principal; Windows/macOS compilan y empaquetan pero **no se han probado en ejecución**.

- **Frontera IPC**: `commands.rs` y módulos `*_cmds`, registrados a mano en `lib.rs` (`generate_handler!`). Ajustes con lista blanca (`ALLOWED_SETTINGS`).
- **Autenticación Moodle**: no hay contraseñas. Login por SSO CAS de la Junta («Acceso Único Educación») dentro de un WebView; sesión detectada por `M.cfg.userId`. Cookies de `*.juntadeandalucia.es` guardadas en el llavero (`session_store.rs`), restauradas al arrancar. Sesión de Moodle caduca a 8 h de inactividad.
- **Extracción**: scripts JS inyectados (`src-tauri/selectors/bridge.js`) en el WebView de Moodle; un único comando accesible desde páginas remotas (`ingest_page`) valida origen/tipo/tamaño; `moodle/mapper.rs` convierte a modelos internos; `sync.rs` persiste (idempotente, soft-delete, rechaza extracciones vacías).
- **Adaptador**: `trait MoodleAdapter { name, list_courses, course_contents }` en `moodle/mod.rs`; solo existe la implementación web. **La interfaz está ligada a Moodle por nombre y tipos** (`RemoteCourse`, `RemoteSection`), no es una abstracción de «integración».
- **Persistencia**: una BD SQLite, 6 migraciones versionadas (`0001`–`0006`), 22 tablas iniciales + `submission_schedule`, `drive_item`, `drive_folder`. Backup con `VACUUM INTO`. Archivos en disco, metadatos en BD (SHA-256, dedupe, versiones).
- **Segundo plano**: `autosync.rs` (bucle + bandeja), `notifier.rs`/`reminders.rs` (avisos con `notify-send`, también modo headless como servicio `systemd --user`), `scheduler.rs` (entregas programadas, cada 20 s).
- **Notificaciones**: tabla `notification` (`category`, `priority`, `ref_kind/ref_id`), `reminders.rs` con ajustes (categorías, horas silenciosas, días, resúmenes). `notif::add` / `reminders::emit` son el punto de entrada.
- **Calendario**: tabla `calendar_event` con `source CHECK IN ('moodle','personal')` y `UNIQUE(source, moodle_event_id)`; `calendar.rs` deriva eventos de actividades y de páginas.
- **Dashboard**: `dashboard.rs` (agregados SQL sobre tareas/eventos/recientes) + `Dashboard.svelte`.
- **Frontend**: navegación por estado compartido (`nav.svelte.ts` + `sections.ts`, lista fija de 9 secciones, sin router); vistas en `src/views`; temas (`themes.ts`, 8 predefinidos + acento propio), `Icon.svelte`, `Palette.svelte` (paleta de comandos), `Welcome.svelte`.
- **Logging**: `tracing` a fichero diario en `~/.local/share/moodledesk/logs`, sin cookies/tokens por diseño (documentado; **no auditado línea a línea**).
- **Otras integraciones ya hechas**: Google Drive (OAuth PKCE, `drive.file`, llavero `auladesk-drive`).
- **Dependencias clave**: tauri 2 (`unstable`, `tray-icon`), sqlx 0.8, reqwest 0.13 (rustls), keyring 3 (libsecret/Windows/Apple), gtk 0.18 (Linux), pdfjs-dist 4.10, svelte 5.57.
- **Documentación**: README, `docs/{ARCHITECTURE,DATABASE,DRIVE,MOODLE_INTEGRATION,SECURITY,ROADMAP}.md`, `PROGRESS.md` (diario muy detallado, incluye lo no verificado). `ARCHITECTURE.md` está **desactualizado** (cita `calendar, notifications, submissions` como «previstos» y ya existen; no menciona Drive/scheduler). No hay `CHANGELOG.md`.

## 2. Estructura del repositorio
```
src/                  Svelte: App.svelte, views/ (12), lib/ (componentes, estado, temas, api.ts)
src-tauri/src/        ~30 módulos Rust planos + moodle/ (adaptador, mapper, window/, session)
src-tauri/selectors/  bridge.js (extractores inyectados, versionados)
src-tauri/migrations/ 0001..0006 .sql
docs/  scripts/  instaladores/  Dockerfile  .github/workflows/
```
Los módulos de dominio están **planos en `src-tauri/src`**; solo Moodle tiene carpeta. No hay `Core/` ni `Integrations/` y no hace falta crearlos de golpe.

## 3. Funcionalidades Moodle existentes (según PROGRESS.md; verificación real indicada allí)
| Área | Estado | Dónde |
|---|---|---|
| Login SSO y sesión persistente en llavero, cerrar sesión y olvidar | Verificado con Moodle real (cierre de sesión: compila, no pulsado) | `moodle/window*`, `session_store.rs`, `bridge.js` |
| Cursos y contenidos (9 cursos, 54 páginas) | Verificado real; cursos ocultos no se extraen | `sync.rs`, `mapper.rs` |
| Biblioteca: descargas con sesión del WebView, hash, dedupe, versiones `_v2`, gestión local, visor (PDF/img/texto/CSV) | Descargas verificadas real (13 archivos); UI de gestión sin pulsar | `downloads.rs`, `library.rs`, `Viewer.svelte` |
| Tareas: fechas, estado, calificación, descripción, adjuntos, tablero | Verificado real salvo descripción/adjuntos nuevos | `tasks.rs`, `Tasks.svelte` |
| Entregas versionadas e inmutables (triggers SQL), entrega desde la app con confirmación, programadas | Lógica probada; entrega real manual verificada, **programada solo lógica** | `submissions.rs`, `scheduler.rs` |
| Calendario, recordatorios 7/3/1 días, resúmenes, horas silenciosas | Tests + uso; quizzes sin fechas | `calendar.rs`, `reminders.rs` |
| Dashboard, bienvenida, temas | Visto en Chromium headless con `invoke` simulado | `dashboard.rs`, `Dashboard.svelte` |
| Drive | Solo contra Google **simulado** | `drive.rs` |
| Calificaciones | Solo en el detalle de tarea (nota/comentarios). **No hay vista de calificaciones por curso.** | `assignment` |

## 4. Componentes reutilizables
- **Notificaciones** (`notif.rs`, `reminders.rs`, `notifier.rs`): genéricas por `category`; reutilizables tal cual para eventos de otras fuentes.
- **Llavero** (`session_store.rs`): lógica de cookies atada a un servicio/usuario y a un filtro de dominio fijo (`relevant()`); el patrón sirve, la implementación hay que parametrizarla.
- **Migraciones, `db.rs`, backup, ajustes, `paths.rs`/`safe_join`, `files.rs`, `downloads.rs`** (dedupe por hash): neutros.
- **Calendario/Dashboard/Tareas en UI**: componentes de presentación reutilizables; el dominio detrás es Moodle.
- **Patrón de puente** (`ingest_page` + `bridge.js` + mapper + sync idempotente): es la plantilla para cualquier integración basada en WebView.
- **Sistema de temas, `Icon`, `Palette`, estados vacíos honestos**: reutilizables.

## 5. Limitaciones arquitectónicas
1. `calendar_event.source` tiene `CHECK IN ('moodle','personal')` y la clave única usa `moodle_event_id`: **no admite nuevas fuentes sin migración** (SQLite no permite alterar un CHECK: hay que recrear la tabla).
2. `user_profile` y `moodle_connection` son **filas únicas** (`CHECK id = 1`): no hay concepto de cuenta vinculada ni de varios alumnos.
3. Estado global de sesión único (`AppState.session`, una `Mutex<SessionState>`) y **un solo WebView con label fijo**; `window.rs` trae `DEFAULT_BASE` y allow-list de Moodle/Junta.
4. Claves de ajustes `moodle_*` sueltas en `application_settings`; whitelist manual de claves.
5. `session_store` guarda **un único secreto** con todas las cookies de `*.juntadeandalucia.es`: con otro portal en el mismo dominio se mezclarían sesiones.
6. Navegación: `sections` es una constante; no hay agrupación por plataforma ni «contexto activo».
7. `lib.rs` crece con cada comando (handler y `setup` monolíticos).
8. Tablas `course/activity/assignment` sin columna de origen: no hay `account_id` en nada.
9. Sin `account_id`, **nada impide mezclar alumnos** (requisito duro de iPasen).

## 6. Problemas detectados
- `ARCHITECTURE.md` desactualizado; falta `CHANGELOG.md`.
- Varias funciones de UI **compiladas pero nunca pulsadas** (lista en PROGRESS.md): conviene cerrarlas antes de construir encima.
- Trabajo de Drive/scheduler sin commit y con 0 verificación contra servicios reales.
- Caducidad real de sesión y cuenta distinta a la sincronizada: no probadas.
- Windows/macOS sin ejecución real.
- Sin tests de frontend; los 80 tests son todos Rust (`--lib`). No existe suite de regresión contra una instancia Moodle (los extractores se prueban con HTML de ejemplo). Hay una prueba marcada `ignored` (no investigada).
- Nombres heredados (`moodledesk`) en BD, llavero y ruta de datos: cambiarlos rompería datos existentes → **no renombrar**.

## 7. Dependencias importantes
Tauri con feature `unstable` (necesaria para webviews hijos; riesgo de cambios entre versiones), WebKitGTK 4.1 (comportamientos propios descritos en PROGRESS), `keyring` por plataforma, `reqwest` solo para Drive. Ninguna nueva es necesaria para el diseño propuesto.

## 8. Riesgos de integración
| Riesgo | Impacto | Mitigación |
|---|---|---|
| iPasen/iSéneca **sin API pública documentada** (ver INTEGRATION_RESEARCH.md) | Alto: el único camino viable es WebView con login legítimo y extracción, frágil ante cambios | Empezar por el que tenga menos fricción; patrón de selectores versionados + rechazo de extracciones vacías (ya probado con Moodle) |
| Mismo dominio/IdP (`juntadeandalucia.es`) para varios portales | Medio: cookies mezcladas en llavero | Un secreto de llavero y un almacén de cookies **por cuenta** |
| Datos de menores (iPasen) y datos de alumnado (iSéneca) | Alto (RGPD/LOPDGDD) | Cifrado en reposo no existe hoy (SQLite en claro): decidir; aislamiento por `account_id`/alumno; sin logs de contenido |
| Autorización de la Junta para iSéneca | Alto: no hay mecanismo autorizado conocido para terceros | Solo lectura, sesión propia del usuario; **no construir escritura** |
| Migración con recreación de `calendar_event` | Medio | Copia de BD previa, test sobre copia, tests Moodle de regresión |
| Estado global de sesión | Medio | Introducir sesión por integración sin cambiar la API de Moodle (ver propuesta) |
| ToS / términos de uso de los portales | Medio **(no verificado)** | Consultar condiciones; consumo humano-equivalente, límites de frecuencia |

## 9. Propuestas de evolución (resumen; el diseño formal será `INTEGRATION_DESIGN.md`, Fase 2)
1. **No reorganizar carpetas**: añadir `integrations/ipasen/` e `integrations/iseneca/` junto a `moodle/`, y un pequeño `integration.rs` con un trait **de capacidades** mínimo (`id`, `status`, `connect`, `disconnect`, `sync`) que Moodle implemente envolviendo lo que hay. Las operaciones propias (entregas, descargas) quedan fuera del trait.
2. **Cuentas**: tabla `linked_account(id, platform, label, state, created_at)` + `student(id, account_id, external_ref, display_name)`. La fila `id=1` actual se mapea como cuenta Moodle por defecto en la migración.
3. **Calendario**: migración que recrea `calendar_event` con `source TEXT` libre, `account_id`, `external_id`, y `UNIQUE(source, account_id, external_id)`; copia los datos con `moodle_event_id → external_id`.
4. **Notificaciones**: añadir `source` y `account_id` a `notification`; mismas reglas de dedupe/horas silenciosas.
5. **Sesión y llavero**: parametrizar `session_store` por `account_id`; un WebView/almacén por integración (patrón `embedded.rs`).
6. **Frontend**: `sections` con grupos (Moodle / iPasen / iSéneca), selector de contexto (cuenta + alumno) en la barra lateral, filtro por plataforma en Dashboard y Calendario, insignia de origen en cada elemento.

## 10. Tabla de componentes (estado real)
| Componente | Estado actual | Acción | Justificación |
|---|---|---|---|
| Autenticación Moodle (SSO en WebView) | Existente, verificada | **Conservar** | Funciona; no maneja contraseñas |
| Sesión persistente (`session_store.rs`) | Existente, 1 secreto global | **Refactorizar** (parametrizar por cuenta) | Evita mezclar sesiones de portales en el mismo dominio |
| Estado de sesión (`AppState.session`) | Único, global | **Refactorizar** | Cada integración necesita su estado |
| `MoodleAdapter` | Existente, atado a Moodle | **Conservar** + trait nuevo de capacidades encima | No romper el mapper/sync |
| Puente WebView (`bridge.js`, `ingest_page`, `mapper`, `sync`) | Existente | **Conservar** como patrón; **Crear** equivalentes por integración | Aislamiento de fallos |
| Sincronización (`autosync.rs`) | Existente, solo Moodle | **Ampliar** | Bucle genérico por integración con su intervalo |
| Base de datos (`db.rs`, migraciones) | Existente, 6 migraciones | **Ampliar** (`0007+`) | Sin recrear la BD |
| `user_profile` / `moodle_connection` | Existentes, fila única | **Refactorizar** → `linked_account` | No hay multicuenta |
| `calendar_event` | Existente, CHECK cerrado | **Refactorizar** (recrear tabla) | Nuevas fuentes |
| `calendar.rs` | Existente | **Ampliar** | Nuevos orígenes |
| Notificaciones (`notif`, `reminders`, `notifier`) | Existentes | **Ampliar** (source, account) | Ya genéricas |
| Dashboard (`dashboard.rs` + vista) | Existente, solo Moodle | **Ampliar** | Agregar por plataforma con filtros |
| Navegación (`nav`, `sections`) | Existente, lista fija | **Ampliar** | Grupos y contexto activo |
| Tema/UI compartida | Existente | **Conservar** | Identidad visual definida |
| Biblioteca/Descargas/Visor | Existente | **Conservar** | Específico de Moodle; iPasen/iSéneca no lo necesitan al inicio |
| Entregas y programadas | Existentes | **Conservar** | Solo Moodle |
| Drive | Existente, sin commit | **Conservar** | Ajeno al alcance |
| Calificaciones (vista por curso) | **No existe** (solo por tarea) | **Crear** | Pedido en el dashboard unificado |
| Búsqueda global | Existe `Palette.svelte` (paleta de navegación; alcance de búsqueda no verificado) | **Evaluar/Ampliar** | — |
| Integración iPasen | No existe | **Crear** | Objetivo |
| Integración iSéneca | No existe | **Crear** | Objetivo (solo lectura) |
| Perfiles/alumnos | No existe | **Crear** | Familias con varios alumnos |
| Tests de caracterización Moodle | Parcial (80 unit, sin e2e) | **Crear** antes de refactorizar | Regla de regresión |
| `ARCHITECTURE.md`, `CHANGELOG.md` | Desactualizado / ausente | **Ampliar / Crear** | Regla de documentación |

## 11. Roadmap propuesto
0. Auditoría (este documento) y decisión sobre el árbol sucio (commit de Drive o rama aparte).
1. Investigación (hecha de forma preliminar, ver INTEGRATION_RESEARCH.md; **falta lo que solo se ve con cuenta real**).
2. `INTEGRATION_DESIGN.md` + tests de caracterización (cadena sync→BD→dashboard de Moodle).
3. Núcleo: `linked_account`, migración de `calendar_event`, `notification.source`, sesión por cuenta. Cada paso con tests Moodle verdes.
4. iPasen (lectura: alumno activo, faltas, calificaciones, comunicaciones) tras un spike de reconocimiento con cuenta de Adrián.
5. iSéneca (solo lectura, condicionado al spike y a que exista acceso con su perfil).
6. Unificación (dashboard, calendario, filtros, búsqueda).
7. Estabilización, Windows/macOS, seguridad, documentación.
