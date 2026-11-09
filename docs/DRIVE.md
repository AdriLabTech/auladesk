# Google Drive y entregas programadas

## Drive
- OAuth 2.0 de aplicación de escritorio: navegador del sistema → redirección a `http://127.0.0.1:<puerto>` → PKCE (S256) + `state`.
- Alcance único: `drive.file` (solo los archivos que AulaDesk sube; no lee el resto del Drive, sin verificación de Google).
- El usuario crea su propio ID de cliente (Google Cloud → Credenciales → «Aplicación de escritorio», API de Drive activada, usuario de prueba = su correo). ID en ajustes; secreto y token de refresco en el llavero (servicio `auladesk-drive`).
- Estructura en Drive: `AulaDesk/<carpetas de rel_path>` (igual que la biblioteca). Mismo contenido (SHA-256) no se sube dos veces; nunca se borra ni sobrescribe nada en Drive.
- «Guardar entregas confirmadas en Drive» (ajuste `drive_backup_submissions`): tras `confirmed` se sube la copia en segundo plano.
- No cubierto: importar a AulaDesk archivos que ya estaban en Drive (exigiría un alcance más amplio).

## Entregas programadas
- Al programar, el usuario confirma una vez (tarea, archivo, SHA-256, hora). Tabla `submission_schedule`.
- `scheduler.rs` revisa cada 20 s. A la hora: `submit_confirmed` (mismo camino verificado que la entrega manual; se detiene si el archivo cambió, la entrega ya tiene archivos o Moodle muestra algo inesperado).
- Sin sesión activa u otra operación en curso: espera y avisa una vez; reintenta cada 10 min, máx. 3 intentos; tras el cierre de la tarea → «missed».
- Necesita la app abierta (o en la bandeja) y sesión de Moodle viva (caduca a las 8 h de inactividad; no se guarda la contraseña).
- Aviso al abrir (`PendingPrompt.svelte`): lista las entregas preparadas (entregar ahora con confirmación explícita, programar, cancelar programación) y los resultados de programadas no vistos.
