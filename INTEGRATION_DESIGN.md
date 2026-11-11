# INTEGRATION_DESIGN — AulaDesk: Moodle + Familias (iPasen) (+ Docentes/iSéneca aparcado)

Fase 2. Fecha: 2026-11-09. Parte de `PROJECT_AUDIT.md` e `INTEGRATION_RESEARCH.md`. **Es un diseño: no hay cambios de código del núcleo todavía y necesita aprobación antes de la Fase 3.**

## 0. Decisiones tomadas
| Decisión | Origen |
|---|---|
| Autenticación de PASEN/Séneca = **sesión guardada**: el usuario inicia sesión una vez en el WebView; solo se guardan cookies (llavero, una entrada por cuenta). **No se guarda ninguna contraseña.** | Adrián, 2026-11-09 |
| Séneca no comparte la sesión de Moodle (verificado en spike): hay login propio (usuario/clave, QR, certificado). | spike |
| iSéneca = estado `NotAvailable` (sin extractor, sin pantalla de datos). Solo una tarjeta informativa y, como mucho, abrir el portal. | Adrián, 2026-11-09 |
| Solo lectura. Justificar faltas, calificar, etc. quedan fuera. | prompt §7, §18 |
| **BD cifrada con SQLCipher** (hecho en esta rama, ver `docs/SQLCIPHER.md`). Cierra la decisión pendiente de cifrado. | Adrián, 2026-11-09 |
| **Nombres públicos**: PASEN/iPasen → **«Familias»**, iSéneca → **«Docentes»** en UI y código visible; PASEN/Séneca solo como texto descriptivo en documentación. Ver `docs/LEGAL.md`. | Adrián, 2026-11-09 |
| Sin renombrar nada heredado (`moodledesk.db`, servicio de llavero `moodledesk`). | auditoría |

## 0 bis. Decisiones del 2026-11-11 (implementadas en la rama `integracion-familias`)
- Familias se lee con un **WebView propio y oculto** (como Moodle), extrayendo el DOM, y se implementarán **todas las opciones que el portal permita**, con interfaz propia. Las de escritura (justificar faltas, pedir tutoría, mensajes) existen como capa con confirmación explícita y registro, pero **no llegan al portal** hasta que haya una receta verificada contra el DOM real.
- **iSéneca/Docentes aparcado**: no hay forma de obtener su DOM; solo una entrada informativa «no disponible».
- Implementado: H0 (tests de caracterización + test de migración sobre BD 0006 con datos) · H1 (cuentas, contrato `Integration`) · H2 (llavero y estado de sesión por plataforma) · H3 (calendario multi-origen, avisos con origen y dedupe) · H4 (webview de Familias) · H5 (contrato de datos, persistencia, recetas; **extractores pendientes del reconocimiento**) · H6 (UI por grupos, alumno activo, filtros) · H7 (docs, límites de frecuencia) · H8 (acciones de escritura con confirmación).
- Desviaciones del diseño original: sqlx envuelve cada migración de SQLite en una transacción, por lo que `0008` conserva los avisos con una tabla auxiliar en vez de apagar `foreign_keys`; los nombres de plataforma en BD son `moodle|familias|docentes`; `calendar_event` mantiene `moodle_event_id` para Moodle y usa `external_id` para el resto.

## 1. Principios
1. Moodle no cambia de comportamiento: cada paso del núcleo se hace con los tests Moodle en verde y con migraciones sobre copia de la BD.
2. Aislamiento: un fallo de PASEN no puede parar la sincronización de Moodle (tareas y estado separados, errores propios).
3. Nada de mezclar alumnos: toda fila de PASEN lleva `account_id` y `student_id`; las consultas siempre filtran por alumno activo.
4. Datos no inventados: si una integración está desconectada o caducada, la UI lo dice y muestra la fecha de la última sincronización en lugar de datos como actuales.

## 2. Modelo de datos (migraciones `0007`+, versionadas y aditivas)
**0007_accounts.sql**
```sql
CREATE TABLE linked_account (
  id INTEGER PRIMARY KEY,
  platform TEXT NOT NULL CHECK (platform IN ('moodle','familias','docentes')),
  label TEXT NOT NULL,                 -- nombre visible
  external_user TEXT,                  -- id/nombre que muestra el portal, nunca credenciales
  state TEXT NOT NULL DEFAULT 'disconnected',  -- connected | expired | disconnected | unavailable
  last_ok_at INTEGER, last_error TEXT,
  created_at INTEGER NOT NULL,
  UNIQUE (platform, external_user)
);
INSERT INTO linked_account(platform,label,external_user,state,created_at)
  SELECT 'moodle','Moodle', moodle_user_id, 'disconnected', strftime('%s','now') FROM user_profile WHERE id=1;
CREATE TABLE student (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES linked_account(id) ON DELETE CASCADE,
  external_ref TEXT NOT NULL,          -- clave estable que dé el portal (o hash del nombre si no hay id)
  display_name TEXT NOT NULL, center TEXT, course_group TEXT,
  UNIQUE (account_id, external_ref)
);
```
Los datos Moodle existentes **no cambian de tabla**; se asumen de la cuenta Moodle (id fijo creado en la migración). No se añade `account_id` a `course/activity/assignment` hasta que haga falta una segunda cuenta Moodle (YAGNI).

**0008_calendar_sources.sql**: recrear `calendar_event` (SQLite no permite cambiar un CHECK) con `source TEXT NOT NULL` libre (`moodle|personal|familias`), `account_id`, `student_id` NULL, `external_id TEXT`, `UNIQUE(source, account_id, external_id)`. Copia: `moodle_event_id → external_id`, `account_id = cuenta Moodle`. Reconstruir índices y FKs de `reminder`. **Migración de mayor riesgo; se prueba sobre copia con el esquema y datos reales.**

**0009_notifications_source.sql**: `ALTER TABLE notification ADD COLUMN source TEXT`, `account_id`, `student_id`, `dedupe_key TEXT` + índice único parcial sobre `dedupe_key` para evitar duplicados.

**0010_familias.sql** (en Fase 4, tras el spike): `attendance_record`, `grade_record`, `school_message`, cada una con `student_id`, `external_id`, `content_hash`, `removed_at`, `first_seen_at` (mismo patrón de sync idempotente y soft-delete que Moodle). El esquema concreto depende de lo que muestre el DOM real; **no se fija hasta el spike autenticado**.

Backups: antes de cualquier migración, `VACUUM INTO exports/pre-migracion-<n>.db` (ya existe la función de backup).

## 3. Contrato de integración (Rust)
Nuevo `src-tauri/src/integration.rs`, mínimo y por capacidades; Moodle no necesita reescribirse.
```rust
#[async_trait]
pub trait Integration: Send + Sync {
    fn platform(&self) -> Platform;                 // Moodle | IPasen | ISeneca
    async fn status(&self, account: i64) -> IntegrationStatus; // Connected/Expired/Disconnected/Unavailable{reason}
    async fn sync(&self, account: i64) -> AppResult<SyncReport>; // lectura; idempotente
    async fn disconnect(&self, account: i64) -> AppResult<()>;   // borra cookies/llavero/datos de esa cuenta
}
```
- No hay `authenticate()` en el trait: el login es interactivo en el WebView (visible), cada integración expone su propio «abrir para iniciar sesión».
- Capacidades extra (entregas de Moodle, alumno activo de PASEN) son métodos propios del módulo, no del trait.
- `IntegrationRegistry` en `AppState` con una entrada por plataforma; `ISeneca` devuelve siempre `Unavailable { reason: "Sin acceso verificado" }`.
- `MoodleIntegration` envuelve lo existente (`window`, `session`, `sync`) sin moverlo. El trait `MoodleAdapter` se queda.

## 4. Sesión, WebView y llavero
- `session_store.rs` pasa a recibir `account_id` y guarda en la entrada de llavero `moodledesk / moodle-session-cookies` (la actual, para no perder sesiones) y `moodledesk / session-<account_id>` para las nuevas. Filtro de dominio por plataforma (`relevant(platform, domain)`): Moodle y CAS por un lado; `seneca.juntadeandalucia.es` por otro, así no se mezclan.
- Un WebView por integración (patrón `moodle/window/embedded.rs`, label `ipasen`), con almacén de cookies propio y allow-list propia (`seneca.juntadeandalucia.es` + CAS). Moodle se queda con su label y su allow-list.
- `AppState.session` (único) → `HashMap<account_id, SessionState>` tras un wrapper con los mismos métodos para que el código de Moodle no cambie.
- IPC remoto: **nuevo** comando `ingest_ipasen` con capability limitada al origen exacto de PASEN. `ingest_page` de Moodle no se toca. El payload nunca se ejecuta, igual que hoy.
- Cierre: «Desconectar PASEN» borra cookies, entrada de llavero y datos de esa cuenta.

## 5. iPasen (Fase 4; alcance real tras el spike)
- Entrada: portal `seneca.juntadeandalucia.es` → «PASEN Familias». Extractores `selectors/ipasen.js` versionados + comprobación de estructura; extracciones vacías o con estructura rara se **rechazan** sin tocar la BD (como en Moodle).
- Alcance inicial (solo lectura): lista de alumnos de la cuenta y selector, calificaciones, faltas/retrasos, observaciones, comunicaciones/avisos, tutorías. Lo que el DOM real no permita se marca «no disponible», no se simula.
- Alumno activo: estado global `activeStudentId`; todos los comandos IPC de PASEN lo reciben explícito (no estado implícito) y validan que pertenezca a la cuenta.
- Calendario: tutorías y eventos como `source='ipasen'`, `external_id` estable.
- Notificaciones: faltas nuevas, calificaciones nuevas, mensajes nuevos con `dedupe_key = ipasen:<student>:<tipo>:<external_id>`; primera sincronización silenciosa (igual que en Moodle).
- Frecuencia: un sync por hora como máximo y siempre con la sesión del propio usuario; sin sondeo agresivo.

## 6. Interfaz
- `sections.ts` gana un campo `group: 'moodle'|'ipasen'|'core'` y una sección «PASEN» (Resumen, Calificaciones, Faltas, Comunicaciones) que solo aparece con la cuenta conectada; «iSéneca» como entrada informativa con estado «No disponible».
- Selector de contexto en la barra lateral: cuenta + alumno activo; el contexto activo se ve siempre en la cabecera de la vista.
- Dashboard y Calendario: filtros por plataforma (chips) y una insignia de origen en cada elemento. Si una integración está caducada/desconectada su bloque muestra «Sin sesión · última sincronización <fecha>».
- Se reutilizan `Icon`, `Palette`, temas y componentes existentes; nada de componentes duplicados.

## 7. Estrategia de pruebas
1. **Antes de tocar el núcleo**: tests de caracterización de Moodle: (a) migraciones desde `0001`..`0006` con datos de ejemplo hasta `0010` conservando contenido; (b) sync → `calendar_event` → dashboard; (c) sesión (clasificación de URL, almacenamiento de cookies con llavero simulado); (d) notificaciones y recordatorios.
2. Cada migración: test sobre copia con datos de Moodle y comprobación de conteos antes/después.
3. PASEN: mapper y sync con **páginas HTML de ejemplo claramente marcadas como simuladas** (y anonimizadas); pruebas de «no mezclar alumnos» y de rechazo de extracciones vacías.
4. Frontend: `svelte-check` + comprobación visual en Chromium con `invoke` simulado, como hasta ahora.
5. Baseline hoy: 80 tests Rust pasan (1 ignorado). El criterio es que ese número no baje.

## 8. Orden de implementación (cada hito deja la app arrancable)
| Hito | Contenido | Riesgo |
|---|---|---|
| H0 | Tests de caracterización de Moodle + test de migraciones sobre copia | Bajo |
| H1 | `0007` + `integration.rs` + registro; Moodle envuelto; iSéneca `Unavailable` | Bajo |
| H2 | `session_store` por cuenta (entrada actual conservada) y estado de sesión por cuenta | Medio |
| H3 | `0008` calendario + `0009` notificaciones | **Alto** (recrear tabla) |
| H4 | WebView `ipasen`, login asistido y persistencia de sesión | Medio |
| H5 | Spike autenticado → esquema `0010` → extractores → sync → IPC | Medio-alto (depende de la web real) |
| H6 | UI: grupos, selector de alumno, filtros, Dashboard/Calendario | Medio |
| H7 | Estabilización, Windows/macOS, docs (`ARCHITECTURE.md`, `CHANGELOG.md`) | — |

## 9. Fuera de alcance / pendiente
- iSéneca (sin acceso verificado ni perfil docente para probar).
- Escritura en PASEN (justificar faltas).
- Cifrado de ficheros descargados y logs (la BD ya está cifrada).
- **Integrar la rama de Drive** (migración 0006 ya aplicada en tu BD real) antes de usar esta rama con datos reales.
- Autorización de la Junta / términos de uso: sin verificar.

## 10. Qué necesito para empezar
1. Tu OK a este diseño (o cambios).
2. Para H5: que inicies sesión tú en el WebView de PASEN durante el spike (yo no introduzco credenciales).
