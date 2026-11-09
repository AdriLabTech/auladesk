# Copia en la nube y entregas programadas

## Copia en una carpeta sincronizada (`folder_backup.rs`)
El usuario elige una carpeta que ya sincronice su nube (Google Drive para escritorio, OneDrive, Dropbox, Nextcloud, Syncthing…) y AulaDesk copia ahí `AulaDesk/<estructura de la biblioteca>`. Sin cuentas, claves ni permisos de Google.
- Nunca sobrescribe ni borra: mismo SHA-256 = se omite; contenido distinto con el mismo nombre = sufijo `_vN`.
- Opcional: copiar cada entrega confirmada (ajuste `backup_folder_submissions`).
- La carpeta no puede estar dentro de la carpeta de datos de AulaDesk y debe poder escribirse (se comprueba al elegirla).
- En Linux no hay cliente oficial de Drive: sirve una carpeta de `rclone mount`, Insync u otra nube.
- **Historia:** hubo una conexión directa con Google Drive (OAuth con ID de cliente propio). Se **eliminó el 2026-11-09 por complejidad** (cada usuario tenía que crear un proyecto en Google Cloud). Las tablas `drive_item` y `drive_folder` de la migración `0006` quedan sin uso: las migraciones aplicadas no se pueden editar.

## Entregas programadas
- Al programar, el usuario confirma una vez (tarea, archivo, SHA-256, hora). Tabla `submission_schedule`.
- `scheduler.rs` revisa cada 20 s. A la hora: `submit_confirmed` (mismo camino verificado que la entrega manual; se detiene si el archivo cambió, la entrega ya tiene archivos o Moodle muestra algo inesperado).
- Sin sesión activa u otra operación en curso: espera y avisa una vez; reintenta cada 10 min, máx. 3 intentos; tras el cierre de la tarea → «missed».
- Necesita la app abierta (o en la bandeja) y sesión de Moodle viva (caduca a las 8 h de inactividad; no se guarda la contraseña).
- Aviso al abrir (`PendingPrompt.svelte`): lista las entregas preparadas (entregar ahora con confirmación explícita, programar, cancelar programación) y los resultados de programadas no vistos.
