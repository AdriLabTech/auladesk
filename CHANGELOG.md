# Cambios

## 0.2.1 — 2026-11-09
- Arreglo de compilación en Windows y macOS: SQLCipher se compila con SQLite incluido (sin depender de cabeceras del sistema). La 0.2.0 solo se compiló bien en Linux.

## 0.2.0 — 2026-11-09
- **Base de datos cifrada** con SQLCipher (clave aleatoria en el llavero del sistema). Una BD existente se cifra sola al abrir la nueva versión y deja la copia sin cifrar como `moodledesk.db.sin-cifrar` (bórrala cuando compruebes que todo va bien).
- **Copia en una carpeta sincronizada** (Google Drive para escritorio, OneDrive, Dropbox…): copia la biblioteca y, opcionalmente, cada entrega confirmada, sin cuentas ni permisos.
- **Entregas programadas** y aviso al abrir con las entregas preparadas.
- Documentación de auditoría, diseño y base de datos cifrada (`docs/`, `PROJECT_AUDIT.md`, `INTEGRATION_*.md`).
- Se descartó la conexión directa con Google Drive por su complejidad.
- Al actualizar, cierra también el servicio `auladesk-notifier` antiguo: el binario viejo no lee la BD cifrada.

## 0.1.1
- Tablero de tareas arrastrable y Moodle como webview dentro de la ventana principal en Linux.
