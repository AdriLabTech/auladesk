> Diario de desarrollo (primera persona, en orden cronológico). Las pruebas contra Moodle se hicieron con la cuenta del autor; no se incluyen datos personales.

# PROGRESS
**Fase actual:** 5 completada y verificada en lo esencial; 6 con backend hecho y probado (59 tests) pero SIN interfaz; 7 sin empezar. Actualizado 2026-11-07. Sesión persistente hecha. Actualizado 2026-11-07.

## Hecho (verificado)
- Proyecto Tauri 2.12 + Svelte 5 compila (`npm run build`, `svelte-check` 0 errores, `tauri build --debug --no-bundle`).
- 8 tests Rust pasan (`cargo test --lib`): hash SHA-256, no-sobrescritura, anti-traversal, saneado, migraciones, ajustes, backup, historial de entregas inmutable, login≠sesión.
- Arranca en Hyprland/Wayland (monitor eDP-2, ws 2), crea estructura XDG, BD con 22 tablas y log.
- UI: sidebar contraíble, topbar, tema oscuro/claro, Configuración (ajustes persistentes, copia de seguridad, diagnóstico). Resto de secciones: estado vacío honesto.

## Fase 2 (verificado contra el Moodle real)
- Login por SSO «Acceso Único Educación» en el WebView embebido (cookies en su propio almacén). Sesión detectada por `M.cfg.userId`.
- Sincronización real: 9 cursos → 9 OK, 0 fallos; segunda pasada 0 cambios; datos locales (favoritos) y soft-delete cubiertos por tests (15 tests).
- Selectores v1 versionados + comprobación de estructura; extracciones inválidas/vacías se rechazan sin tocar la BD.
- Canal de control de desarrollo (`AULADESK_DEV_CTL=1`), apagado por defecto.
- Defectos encontrados y corregidos: bloqueo del IdP por mi propia lista de navegación; patrón `remote.urls` con ruta (Tauri ve origen sin ruta); mi afirmación errónea de «sin SSO».

## Fase 3 — Biblioteca (verificado contra el Moodle real, 2026-11-07)
- Descarga con la sesión del WebView: 13 archivos reales (PDF, ZIP, 6 `.cs` de una carpeta, enunciados de tareas) guardados en `files/<Asignatura>/<Tema_NN|General>/<Recursos|Actividades>/`. Cada uno es el archivo correcto de su actividad; hash en disco = hash en BD (comprobado en el PDF, válido de 8 págs.); 0 huérfanos, 0 temporales.
- Persistencia: tras reiniciar el proceso, los 13 archivos y la BD siguen ahí (Biblioteca los lista).
- Ruta de la interfaz: botón «Descargar» pulsado con el teclado → la app muestra el error en rojo cuando una tarea no tiene adjuntos del profesorado (comprobado en la página: solo tenía mi propia entrega).
- Integridad (tests): HTML disfrazado de PDF rechazado y sin restos; mismo hash = no se duplica, solo se asocia; contenido nuevo = `_v2` y la anterior `outdated`; archivo descargado «a ciegas» se adopta y se mueve a su carpeta.
- Hallazgos de Moodle/WebKit: un recurso redirige directo al PDF y WebKit lo muestra en su visor (sin descarga) → se pide `forcedownload=1`; WebKit emite «página cargada» antes de iniciar la descarga y eventos rezagados de la anterior; el nombre sugerido por el navegador añade « (1)» si ya existe en Descargas → se usa el nombre de la URL; Moodle manda tildes en NFD → se normaliza a NFC.
- Bugs míos hallados y corregidos con pruebas reales: carrera de eventos (asoció el PDF a la carpeta equivocada), cierre de ventana de Moodle (ahora se oculta), consulta SQL con `?`/`?1` mezclados.
- Rediseño: tarjetas con portada (8 SVG generados por Moodle + 1 imagen real reducida a 640 px), cabecera con estadísticas, filtros con contadores, índice lateral de temas (oculto <960 px), temas plegables, tareas resaltadas. Vistos en pantalla: lista de temas/tareas con filtro «Tareas», y Biblioteca con datos reales. Atajos Ctrl+1…9.

## Sesión persistente (verificado, 2026-11-07)
- Las cookies de sesión del WebView (solo `*.juntadeandalucia.es`: Moodle + CAS) se guardan en el llavero del sistema (Secret Service / gnome-keyring) y se restauran al arrancar. Comprobado 5 reinicios seguidos: la app vuelve a «Conectado» sin iniciar sesión; sin valores de cookies en logs ni ficheros (WebKit no persiste las cookies de sesión por sí solo).
- Límite del servidor (no resoluble sin guardar la contraseña, descartado): `M.cfg.sessiontimeout` = 28800 s (8 h de inactividad). Pasado ese tiempo hay que volver a entrar. **No he probado** qué ocurre exactamente tras la caducidad real (se espera que la restauración falle, quede «Sin sesión» y se pueda entrar de nuevo).
- Interruptor «Mantener la sesión» y botón «Cerrar sesión y olvidarla» (logout oficial de Moodle con sesskey + borra cookies + llavero): compilan; **no los he pulsado**.
- Hallazgo: WebKit no renderiza las páginas dinámicas de Moodle si la ventana nunca se ha mostrado. Al arrancar con sesión guardada la ventana de Moodle aparece unos segundos y se oculta sola (comprobado: sincronización completa OK tras ese arranque).

## Fase 4 — Tareas y entregas (verificado contra el Moodle real)
- Extractor de la página de tarea (`mod/assign/view.php`): fechas Apertura/Cierre (zona Europe/Madrid, tests con cambio de hora), estado de la entrega y de la calificación, tiempo restante, última modificación, archivos enviados, nota, calificador. Páginas de error o sin estructura se rechazan sin tocar la BD.
- Sincronización real: 40 tareas leídas (25 enviadas, 15 sin envío, 11 calificadas), 30 con fecha límite, 0 errores. Un cambio de fecha límite queda registrado en `sync_history` (`due_changed|…`) para los recordatorios de la Fase 5.
- Estados (derivados al consultar): pendiente, iniciada (borrador), preparada, entregada, evaluada, fuera de plazo, sin fecha (+ sin sincronizar). Caso real cubierto: tarea calificada sin entrega (evaluada). Vista «Tareas»: por hacer / hechas / todas, filtros, orden por fecha, prioridad, asignatura o estado, prioridad personal, favorita, «hecha» local y nota. «Urgente» solo por fecha límite < 48 h y sin entregar. Las vencidas hace >30 días (restos de cursos pasados) van al final y atenuadas.
- Entregas: preparar = copia propia versionada en `<Asignatura>/<Tema>/Entregas/` (la copia no cambia si el original se edita; mismo contenido activo rechazado; versión N+1 sin pisar). Historial inmutable (triggers) + estados prepared / in_progress / pending_confirmation / confirmed / error.
- Regla de confirmación: `confirmed` SOLO si Moodle indica «Enviado…» y el archivo que muestra tiene el mismo SHA-256 que la copia preparada. Probado contra Moodle real, solo lectura y en la BD de pruebas: (a) tu archivo ya entregado → confirmada, con evidencia, fecha de envío de Moodle y nota; (b) un archivo distinto → NO confirmada («Moodle muestra otro contenido»).
- «Entregar en Moodle…» exige confirmación explícita (diálogo + parámetro `confirmed`), abre la página oficial de edición y **no envía nada**: adjuntas y envías tú en Moodle.

## Entrega real de extremo a extremo (2026-11-07) — criterios 10 y 11
- Tarea una tarea real de prueba: un PDF preparado en AulaDesk (versión 1, con su SHA-256), página oficial de entrega abierta por la app tras confirmación explícita del usuario, archivo adjuntado y enviado **por el usuario** en Moodle. La app NO envió nada.
- Comprobación posterior (solo lectura): Moodle indicó «Enviado para calificar» y el archivo que muestra tiene el mismo SHA-256 que la copia preparada → versión `confirmed`, con evidencia y fecha de envío de Moodle.
- Hallazgos: (1) Tauri/wry en Linux capturaba los archivos soltados y la página no los recibía → `disable_drag_drop_handler()` en la ventana de Moodle (necesario, pero el arrastre desde el gestor de archivos del usuario siguió sin funcionar en la prueba; causa no confirmada); (2) la vía del botón «Agregar… → Subir un archivo → Elegir archivo» sí está disponible; (3) la ventana de Moodle puede abrirse en otro monitor: moverla al portátil.
- Sigue sin verificar: el cuadro «Elegir archivo» abriendo el selector del sistema dentro de la ventana (no llegué a verlo), y los botones de preparar/comprobar de la interfaz (probados por el canal de desarrollo).

## Fase 5 — Calendario y notificaciones (verificado con datos reales de una cuenta de prueba)
- Calendario: 30 eventos de entrega y 4 de cuestionarios derivados de las fechas de las actividades; los de la vista «Próximos eventos» de Moodle no se duplican (se cruzan por cmid). Eventos personales editables; los de Moodle solo admiten notas locales, que sobreviven a cambios de fecha (tests). Vista de mes vista en pantalla con eventos reales; semana, día, agenda y el formulario de evento personal: sin ver.
- Recordatorios por reglas (7/3/1 días, configurables): cancelados al entregar, rearmados si cambia la fecha, un solo aviso inicial en vez de una avalancha, franja silenciosa y días permitidos (aplaza, no pierde), categorías, resúmenes diario/semanal. 8 tests de reglas + 2 de sincronización.
- Fallo propio encontrado por un test: la primera sincronización habría lanzado «Nueva tarea» por todo lo existente; corregido.
- Notificaciones de escritorio por Freedesktop (`notify-send`): vista una real en tu shell. Modo `--notifier` sin ventana probado junto a la app: 1 aviso real, sin duplicados tras un ciclo de la app. Unidad de systemd validada con `systemd-analyze`; **NO instalada** (botón en Notificaciones → Preferencias, solo si tú lo pides).
- Sin verificar: la pestaña Preferencias y la bandeja con varias entradas, resúmenes diario/semanal con la hora real, instalación real del servicio, cuestionarios futuros / exámenes reales (los 2 cuestionarios existentes son de octubre), eventos de curso de la vista «Próximos eventos» (los 3 existentes eran tareas).

## Fase 6 — Dashboard y rediseño visual (2026-11-08)
- Backend `dashboard_data` (3 tests): contadores, prioridades con puntuación transparente, próximos eventos, actividad reciente, progreso por asignatura.
- `Dashboard.svelte` hecho: saludo con resumen real, anillo de progreso, 4 KPIs con contador animado, prioridades (clic abre la tarea), próximos 14 días, progreso por asignatura, actividad reciente; estado vacío si no hay datos.
- Rediseño de toda la app: sistema de diseño global en `app.css` (tokens oscuro/claro, cristal, foco que sigue al puntero, botones, chips, campos, badges), iconos SVG propios (`lib/Icon.svelte`), barra lateral con indicador deslizante, paleta de comandos Ctrl+K (secciones, asignaturas, tareas), transiciones entre secciones, entradas escalonadas, `prefers-reduced-motion` respetado.
- Verificado: `svelte-check` 0 errores, `vite build`, `tauri build --debug`. Visto en Chromium headless con `invoke` SIMULADO (datos de muestra, no reales): Dashboard oscuro y claro, Tareas, Asignaturas, Biblioteca, Calendario, paleta.
- NO verificado: la app Tauri real con el nuevo frontend (no la lancé para no tapar tu workspace), rendimiento de `backdrop-filter` en WebKitGTK, animaciones a velocidad real, Notificaciones/Conexión/Configuración/Entregas y el detalle de asignatura restilados sin ver, vista estrecha (<1000 px).

## Fase 7 — Optimización: sin empezar (memoria, arranque, consultas SQLite, accesibilidad, Hyprland).

## No verificado / pendiente
- «Abrir», «Carpeta» (gestor de archivos), «Renombrar», «Mover», «Eliminar», «Importar», crear carpeta, duplicados y el tema claro: compilan y los tests cubren la lógica de BD/ficheros, pero **no los he pulsado en la interfaz**. «Abrir» usa el visor predeterminado del sistema (no comprobado).
- Vista de cuadrícula de asignaturas con portadas e índice lateral con ventana ancha: no vistas (solo con ventana estrecha). El indicador de sesión de la barra superior se corrigió tras verlo desactualizado; la corrección no se ha visto con sesión activa.
- Botones «Preparar entrega» (diálogo de archivo), «Comprobar entrega», «Cancelar» y el diálogo de confirmación: la lógica está probada por el canal de desarrollo y por tests; **no los he pulsado en la interfaz** (sí vi renderizados Tareas, el detalle, y Entregas con historial real).
- Cuestionarios y exámenes: no se extraen fechas de `quiz` todavía (Fase 5).
- Resincronizar tras un cambio real de un archivo en Moodle (ruta `_v2` por la interfaz): solo probado con tests.
- Descargas muy grandes (>100 MB) y cancelación: no probadas.
- La sesión no sobrevive al reinicio de la app (cookie de sesión de Moodle/CAS): hay que volver a entrar. Persistirla sin extraer cookies no está resuelto.
- Fechas de entrega, adjuntos, estados, calificaciones: Fases 3-4. Cursos ocultos (4 de 13) no se extraen.
- Detección de expiración durante una sync larga: solo vía `M.cfg.userId`; no probada forzando caducidad.
- Persistencia de tema/sidebar tras reiniciar y botón de backup: probados solo a nivel de test Rust, no clicando en la UI.
- Todo Moodle real (Fases 2–7). Criterios de aceptación 1–15: **ninguno completado** salvo el 1 en parte (arranca; falta login).
- Tema claro y escalado fraccionario sin ver.

## Decisiones
Ver docs/ARCHITECTURE.md y MOODLE_INTEGRATION.md. npm en vez de pnpm (no instalado).

## Limitaciones de Moodle
Sin token; extracción web. Versión 5.x por `apibase`. Detalle en MOODLE_INTEGRATION.md.

## Identidad visual v2 (2026-11-08)
- Adiós al degradado índigo→cian, fondo aurora y cristal. Ahora: superficies planas, bordes finos, acento saturado, cabecera del Dashboard a todo color, retícula de puntos tenue, tipografía Space Grotesk (títulos y cifras) + Inter + JetBrains Mono.
- 8 temas predefinidos (`lib/themes.ts`): Brasa (por defecto), Lima, Cobalto, Fucsia, Menta, Señal (oscuros), Papel (claro, títulos en Fraunces) y Hielo (claro). Cada tema fija 6 colores; el resto se deriva en `app.css`.
- Personalización en Configuración: acento propio (el texto sobre él se ajusta por luminancia), esquinas recto/normal/redondo y modo tranquilo (sin animaciones). Se guarda en los ajustes (`theme_id`, `theme_accent`, `theme_radius`, `theme_calm`); el ajuste antiguo `theme` claro/oscuro se migra (claro→Hielo, oscuro→Brasa).
- Verificado en Chromium headless con `invoke` simulado: Brasa, Lima, Papel, Cobalto (Configuración), Fucsia (Tareas), Señal+Recto (Configuración). NO visto: Menta/Hielo completos, el selector de acento y el modo tranquilo en la app real, persistencia tras reiniciar (solo compila), Courses/Biblioteca/Calendario/Notificaciones/Conexión con los temas nuevos, contraste de los estados (azul/verde/ámbar) sobre cada tema.

## Cuenta visible (2026-11-08)
- Tras iniciar sesión se lee el nombre real de la cuenta: primero del menú de usuario de la página (`.usermenu .usertext`…) y, si el tema no lo trae, del perfil propio (`/user/profile.php?id=<M.cfg.userId>`, mismo origen). Se saneó en Rust (`clean_name`, 2–120 caracteres, sin `<>`; 5 casos en test) y se guarda en `application_settings` (`moodle_user_id`, `moodle_user_name`), no en logs. «Cerrar sesión y olvidarla» lo borra; si entra otra cuenta y no llega su nombre, se borra el anterior.
- Se muestra en: tarjeta de sesión de la barra lateral (avatar con iniciales + nombre + estado), cabecera del Dashboard («Sesión de … · ID …») y tarjeta de cuenta en Moodle / Conexión.
- Verificado: 60 tests Rust; el puente JS en Chromium con dos páginas de ejemplo (con menú de usuario y sin él, forzando el perfil); la interfaz con una cuenta SIMULADA («Ana López de Prueba»).
- NO verificado contra un Moodle real (tema «moove»: los selectores del menú son los estándar de Moodle 4/5, no los he visto en la página real; el perfil es el respaldo). Si no sale el nombre, la tarjeta de Conexión lo dirá. Tampoco se detecta aún que entre una cuenta distinta de la que sincronizó los datos (solo se actualiza el nombre).

## Bienvenida y tablero de tareas (2026-11-08)
- **Bienvenida** (`lib/Welcome.svelte`): al PASAR a sesión activa (no en cada recarga de página) aparece a pantalla completa con el color del tema: revelado circular, avatar con iniciales y anillo que se dibuja, check, nombre real por palabras, y cifras reales del Dashboard (tareas por hacer, vencidas, exámenes). Se cierra sola a los ~4,8 s o con clic/Esc/Enter. No sale en modo tranquilo ni con «reducir movimiento». Configuración → «Animación de bienvenida»: botón «Ver ahora» y interruptor (`welcome_enabled`).
- **Tablero tipo Trello** en Tareas (botón Lista | Tablero, se recuerda): columnas por Estado (Vencidas, Por hacer, En curso, Sin fecha, Entregadas, Evaluadas), por Asignatura o por Prioridad. Solo el tablero de Prioridad permite arrastrar (cambia tu prioridad local); por Estado/Asignatura no, porque el estado lo manda Moodle. Clic en una tarjeta abre un cajón de detalle con las mismas acciones que la lista (más un selector de prioridad). Las tareas antiguas (>30 d) se ocultan salvo «Antiguas».
- **Fallo corregido**: las animaciones de entrada usaban `fill-mode: both`, que deja un `transform` activo y hacía que todo `position: fixed` interno quedara recortado a la tarjeta/vista (afectaba al diálogo «Entregar en Moodle…» dentro de una tarea). Ahora usan `backwards`.
- Verificado (Chromium headless, `invoke` simulado): tablero por estado, por prioridad (con tarjeta flotante y columna resaltada), por asignatura (sin revisar la captura), cajón de detalle, y la bienvenida en 3 instantes y 2 temas (Brasa, Lima). El arrastre, con eventos de puntero SINTÉTICOS: soltar la tarea 8 en «Prioridad baja» llama a `task_set_flags` con `priority: 1`.
- NO verificado: el arrastre con ratón real en WebKitGTK/Tauri, la bienvenida real tras un inicio de sesión (solo la vista previa), fluidez de la animación a velocidad real, el diálogo de entrega tras el arreglo de `fixed`, y el tablero con muchas tarjetas (rendimiento).

## Fix: los temas no se guardaban (2026-11-08)
- Causa: `set_setting` solo aceptaba 4 claves (`ALLOWED_SETTINGS` en commands.rs); `theme_id`, `theme_accent`, `theme_radius`, `theme_calm`, `welcome_enabled`, `tasks_view` y `tasks_board_by` se rechazaban y el error se perdía en un `.catch(console.error)`. Por eso siempre volvía Brasa (y la vista Lista/Tablero tampoco se recordaba).
- Arreglo: claves añadidas + test `frontend_setting_keys_are_allowed` (61 tests OK). Comprobado por inspección que las 10 claves que usa el frontend están permitidas.
- NO verificado en la app real: guardar y reiniciar (hay que elegir un tema, cerrar la app y volver a abrirla).

## Paquete «siguientes pasos» (2026-11-08)
- **Información completa de la tarea**: el extractor lee la descripción (texto plano con viñetas, nunca HTML) y los enlaces https de la página de la tarea; migración 0005. El detalle muestra descripción, enlaces/adjuntos, archivos descargados (con «Ver»), calificación y comentarios, estado/fechas y «todos los datos tal cual los muestra Moodle». Probado con una página de ejemplo (viñetas OK, `javascript:` descartado). NO visto con tu Moodle real.
- **Visor integrado** (`lib/Viewer.svelte`, `file_read` en Rust con tope de 80 MB y ruta resuelta por `safe_join`): PDF con pdf.js 4.10 (Apache-2.0, carga diferida), imágenes, texto/código, CSV. Probado en Chromium con un PDF real de 2 páginas. NO soporta Office/zip (se ofrece «Abrir con el sistema»). Pendiente: texto seleccionable/búsqueda en PDF.
- **Sincronización automática** (`autosync.rs`): cada 10 min–12 h (30 por defecto), solo con sesión activa y sin otra operación en curso; reintento a mitad de intervalo tras un fallo. 2 tests de la regla. Ajustes en Moodle / Conexión.
- **Bandeja del sistema** (`tray-icon`): cerrar la ventana la oculta y la app sigue sincronizando. En este equipo NO hay `libayatana-appindicator`, así que la creación está protegida (`catch_unwind`) y, si falla, cerrar la ventana cierra la app. NO verificada visualmente.
- **Cartel del Dashboard** personalizable (`DashHero.svelte`, `herocfg.svelte.ts`): título y subtítulo con {nombre}/{tareas}/{vencidas}, 5 interruptores, color acento/neutro, vista previa en vivo y restablecer.
- **Entregar desde la app** (`submission_submit` + etapas `submit_*` en el puente): comprueba que el archivo en disco es el preparado (SHA-256), abre el formulario, sube el archivo (de Rust a la página en trozos), guarda, envía si hace falta y verifica por SHA-256. Solo tras confirmación explícita; se detiene si la entrega ya tiene archivos o algo es inesperado y deja Moodle abierto. **Fallo real encontrado y corregido en la prueba**: los formularios de Moodle tienen un `<input name="action">` que pisa `form.action` (se lee con `getAttribute`). Probado SOLO contra una página que imita el DOM de Moodle; NO contra un Moodle real; los selectores del selector de archivos y de «Enviar tarea» son los estándar de Moodle 4/5 pero no se han visto en el tema «moove».
- **Empaquetado**: icono propio, `bundle` activo (deb, rpm, appimage, msi, nsis, dmg), llavero por plataforma en Cargo.toml, Dockerfile multi-etapa (`artifacts` y `check`), `scripts/build-docker.sh`, flujos `ci.yml` y `release.yml`. Docker generó `AulaDesk_0.1.0_amd64.deb` (49 MB) y `.rpm` (9 MB). Windows/macOS y los flujos de GitHub Actions NO se han ejecutado.

## Fix: «Migrate(VersionMismatch(1))» al abrir una BD existente (2026-11-08)
- Causa: el renombrado a AulaDesk reescribió un comentario de `migrations/0001_init.sql`; sqlx guarda el checksum de cada migración aplicada y rechaza cualquier cambio.
- Arreglo: restaurado el texto original (checksum idéntico al de la BD real). Test `applied_migrations_are_frozen` fija los checksums de las migraciones 1–4; test manual `opens_existing_database_copy` (ignorado) abre una COPIA de una BD real. Regla: nunca editar una migración existente, ni un comentario.

## Detalle y entrega desde cada asignatura (2026-11-08)
- Las tareas dentro de una asignatura son clicables («Detalle y entrega» + nombre): abren el mismo cajón lateral que el tablero. El detalle se extrajo a `views/TaskDetail.svelte` y el cajón a `lib/TaskDrawer.svelte`, reutilizados por la lista, el tablero y las asignaturas.
- **Modificar entrega**: si la tarea ya tiene archivos entregados, el botón pasa a «Reemplazar entrega desde AulaDesk…» (diálogo con aviso explícito y casilla) y el puente elimina los archivos anteriores en el formulario antes de subir el nuevo; nada se guarda hasta «Guardar cambios», y se detiene sin cambiar nada si no puede borrar. Además, «Modificar entrega en Moodle» abre el formulario oficial de edición.
- Probado SOLO contra una página que imita el DOM de Moodle (borrado con y sin diálogo, con y sin pedir reemplazo); el DOM real del selector de archivos (`.fp-file-delete`, `.fp-dlg-butconfirm`) NO está verificado.

## Fix: la entrega se detenía en «Subir el archivo» (2026-11-08, primera prueba real)
- Síntoma (log): `entrega: etapa submit_stage → detenida` / «No se abrió el selector de archivos». El puente pulsaba «Agregar…» pero no veía el diálogo de subida de Moodle en 8 s.
- Hipótesis (NO confirmada con el DOM real): (1) carrera: el gestor de archivos de Moodle (YUI) engancha sus manejadores tras cargar la página y el clic se daba antes; (2) otro nombre de clase del diálogo en esta versión/tema.
- Cambios: espera a `readyState=complete` + 1,5 s, hasta 4 intentos de clic, detección del diálogo con varios selectores, y si falla el mensaje incluye `[diag: …]` con el HTML del botón y los nombres de clase de la página (nunca contenido). Mensajes de etapa hasta 900 caracteres.
- Probado en la página de ejemplo: normal, manejador tardío (2,5 s), diálogo con otras clases y «nunca se abre» (devuelve el diag). Sigue sin probarse contra el Moodle real: si vuelve a fallar, el `diag` dirá qué clases hay.

## Entrega real verificada (2026-11-08)
- Con el endurecimiento del paso «Subir el archivo» (espera + reintentos + detección multi-selector), la entrega desde la app **funcionó en el Moodle real** con una tarea real: la app la dio por confirmada (SHA-256) y el usuario la vio entregada en Moodle.
- Verificado así: subida a una tarea SIN archivos previos, flujo guardar/enviar/verificar. Sigue sin verificar contra Moodle real: **reemplazar una entrega ya existente** (borrado de archivos anteriores) y tareas que exigen el paso extra «Enviar tarea».

## Instaladores por GitHub Actions (2026-11-08)
- Flujo `release.yml` ejecutado a mano (workflow_dispatch) y **correcto en las 4 plataformas** (Ubuntu, Windows, macOS Apple Silicon y macOS Intel): borrador de release `v0.1.0` con `.deb`, `.rpm`, `.AppImage`, `.msi`, instalador NSIS `.exe`, dos `.dmg` y los `.app.tar.gz`. Instaladores SIN firmar.
- Primer intento fallido: `Missing export "openViewer"` en Windows/macOS. Causa: `../lib/viewer.svelte` se resolvía al componente `Viewer.svelte` en sistemas de ficheros sin distinción de mayúsculas. Arreglo: módulo renombrado a `viewerstate.svelte.ts`.
- NO verificado: que los instaladores de Windows y macOS ARRANQUEN y funcionen (solo se sabe que compilan y empaquetan). Allí no hay notificaciones de escritorio ni servicio en segundo plano (`notify-send`/`systemd`), y el llavero y el WebView son los del sistema (sin probar).
