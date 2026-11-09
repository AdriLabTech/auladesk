# Google Drive y entregas programadas

## Dos formas de guardar en la nube (de más a menos cómoda)
1. **Carpeta sincronizada (sin configurar nada)** — `folder_backup.rs`. El usuario elige una carpeta que ya sincronice su nube (Google Drive para escritorio, OneDrive, Dropbox, Nextcloud, Syncthing…) y AulaDesk copia ahí `AulaDesk/<estructura de la biblioteca>`. Sin cuentas ni permisos; nunca sobrescribe (mismo SHA-256 = omite, contenido distinto = `_vN`); opcionalmente copia cada entrega confirmada. En Linux no hay cliente oficial de Drive: ahí sirve una carpeta de `rclone mount`/Insync o cualquier otra nube.
2. **Conectar con Google (un clic)** — OAuth directo. Si la compilación trae un cliente de Google propio del proyecto (`AULADESK_GOOGLE_CLIENT_ID` y `AULADESK_GOOGLE_CLIENT_SECRET`, fijados al compilar), el usuario solo pulsa «Conectar con Google». Si no, queda el modo avanzado (cliente propio, abajo).

### Crear el cliente de Google del proyecto (una vez, por el mantenedor)
1. console.cloud.google.com → proyecto nuevo → activar **Google Drive API**.
2. Pantalla de consentimiento OAuth: tipo **Externo**, nombre «AulaDesk», solo el alcance `.../auth/drive.file`, y **pasar a «En producción»**. En «Pruebas» solo entran 100 usuarios de prueba y las autorizaciones caducan a los 7 días. `drive.file` no es un alcance sensible, así que **no debería exigir verificación completa** *(no verificado: confirmarlo en la consola; Google puede pedir verificación de marca)*.
3. Credenciales → ID de cliente de OAuth → **Aplicación de escritorio**.
4. Compilar con las dos variables de entorno (en GitHub: Settings → Variables del repositorio, no «Secrets»: el valor acaba dentro del binario, y Google no considera confidencial el secreto de una app de escritorio).
- Qué no se verificó: nada contra Google real (necesita ese cliente). Todo lo demás está probado con un Google simulado.

### Modo avanzado (cliente propio)
Si el usuario pone su propio ID/secreto, tiene prioridad sobre el integrado. Con la app en «Pruebas» hay que añadir el correo como usuario de prueba y la autorización caduca a los 7 días. Al cambiar de cliente se borra el token anterior (no sirve con otro cliente).

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
