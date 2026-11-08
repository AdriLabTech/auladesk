# Integración con Moodle

## Lo verificado (2026-11-07, peticiones anónimas a páginas públicas)
- `login/index.php` devuelve 200 con formulario propio **y** el botón «Acceso Único Educación» (`login/index.php?authCAS=CAS`), que redirige a `edea.juntadeandalucia.es/cas/login`. (Corrección 2026-11-07: antes se escribió que no había SSO; era falso.) El WebView embebido **funciona** con este SSO si se permite navegar a `*.juntadeandalucia.es`; verificado con login real.
- Config JS de la página: `apibase = …/centros/cordoba/r.php/api` → rutas nuevas de Moodle **5.x**. (`lib/upgrade.txt` ya remite a `UPGRADING.md`, 4.5+.) Versión exacta: no verificada; se leerá con sesión iniciada.
- Cookie de sesión `MoodleSession…` con `Secure; HttpOnly; SameSite=Lax`; balanceador F5 (`BIGip…`, `TS…`) añade cookies propias.
- `webservice/rest/server.php` → `invalidtoken` (dato del usuario).
- `login/token.php` existe y responde `invalidlogin` a credenciales falsas: el servicio móvil **parece** accesible, pero usarlo exigiría que la app maneje la contraseña, lo que el requisito 4.2 prohíbe. No se usa.
- `admin/tool/mobile/launch.php` → 303 a login (requiere sesión).

## Estrategia
1. **Adaptador web (por defecto)**: WebView Tauri con almacenamiento de cookies propio. La extracción se hace **dentro** del WebView (DOM/`fetch` same-origin con la sesión del propio navegador); nunca se sacan cookies fuera.
2. **Fuente complementaria**: calendario iCal exportado; la URL contiene un token → secreto, irá a libsecret.
3. **Adaptador API (futuro)**: solo si hay token legítimo. Posible vía oficial a probar *en Fase 2, con sesión iniciada y a petición del autor*: el flujo de la app móvil oficial (`launch.php` → `moodlemobile://token=…`). Si el admin no lo habilita, no hay API. Funciones a documentar cuando exista token: `core_webservice_get_site_info`, `core_enrol_get_users_courses`, `core_course_get_contents`, `mod_assign_get_assignments`, `mod_assign_get_submission_status`, `core_calendar_get_calendar_events`, `core_files_upload`+`mod_assign_save_submission`/`mod_assign_submit_for_grading`. **Habilitación y permisos: no verificados.**

## Decisión de seguridad: IPC desde páginas remotas
Tauri solo da acceso a comandos a páginas remotas si una capability lista `remote.urls`; en Linux no distingue iframe de ventana principal (docs, Capabilities). Por eso:
- La ventana principal (código propio) **no** carga Moodle; Moodle vive en una ventana/webview aparte.
- Fase 2: se concede a ese origen exacto **un único comando** (`ingest_page`) que valida y acota lo recibido; la UI nunca ejecuta HTML de Moodle.

## Extracción
- Selectores **versionados** (`selectors/v1.json`) + comprobación de estructura (elementos esperados presentes). Si falla: se registra en `sync_history`, se marca el job como fallido y **no se sobrescribe** nada (`content_hash` + `removed_at`, nunca borrado).
- Se conservan `moodle_id`/`cmid` originales. Sin regex sobre HTML: parser DOM en el WebView.
- Descargas: `WebviewBuilder::on_download` (Tauri 2.12.5) con la sesión del WebView. Pendiente de probar con un PDF real.

## Qué requiere API autorizada (no implementable solo con web)
Calificaciones fiables en lote y subida programática; por web se lee la página de cada tarea.
## Entregas
Siempre por la UI web original dentro del WebView, con confirmación explícita del usuario; "confirmada" solo si la página de estado de Moodle muestra el envío (evidencia guardada en `confirmation_evidence`).

## Verificado con sesión real (2026-11-07)
- Detección de sesión: `M.cfg.userId` (0 = sin sesión; >0 = sesión). Un 200 no basta.
- IPC remoto: Tauri ve el origen sin ruta, por lo que `remote.urls` debe ser `https://<host>/*`; la restricción a `/centros/cordoba/` la hace `ingest_page` en Rust.
- `/my/courses.php`: `[data-region=courses-view]` (`data-grouping`, `data-totalcoursecount`), tarjetas `[data-region=course-content][data-course-id]`, nombre en `.multiline[title]`. 9 visibles de 13 (`totalcoursecount`; el resto están «Eliminado de la vista»; no se extraen).
- `/course/view.php?id=N` (format-topics): `li.course-section[data-id][data-number][data-sectionname]`, actividades `li.activity[data-id].modtype_*`, nombre `.instancename` sin `.accesshide`, enlace `.activityname a`. Las etiquetas (`label`) no tienen enlace ni nombre.
- La página de curso **no** trae fechas de entrega ni adjuntos: hay que leer `mod/assign/view.php?id=cmid` y `mod/resource/view.php?id=cmid` (Fases 3-4).
- Resultado: 9 cursos, 40 tareas, 42 recursos, 25 enlaces, 10 carpetas, 9 foros, 2 cuestionarios… Curso 2084: 10 secciones/36 actividades coincide con el volcado de la página.

## Tareas y entregas (verificado 2026-11-07)
- `mod/assign/view.php?id=<cmid>`: fechas en `[data-region=activity-dates]` («Apertura: …», «Cierre: …»); `.submissionstatustable table` (Estado de la entrega / de la calificación / Tiempo restante / Última modificación / Archivos enviados); `.fileuploadsubmission a` + `.fileuploadsubmissiontime`; `.feedback .feedbacktable` (Calificación / Calificado sobre / por); botones «Agregar entrega» / «Editar entrega». Páginas de acceso denegado tienen título «Error».
- Textos en español («Enviado para calificar», «Todavía no se han realizado envíos», «Borrador (no enviado)», «Calificado»/«Sin calificar»). Las filas crudas se guardan (`assignment.raw_rows_json`) por si cambia el idioma o el HTML.
- Los archivos que sube el propio alumno (`assignsubmission_file`) no se tratan como recursos; se usan solo para verificar entregas por hash.
- Una tarea puede estar calificada sin entrega (estado «evaluada»).
