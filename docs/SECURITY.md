# Seguridad
- Sin contraseñas propias: login en el WebView; cookies solo en su almacén.
- Secretos (URL iCal, futuros tokens) en Secret Service; nunca en BD/logs/código.
- Logs sin cookies/tokens/URLs con token. Rutas saneadas y `safe_join`; descargas con `create_new`.
- IPC remoto mínimo y por origen exacto (ver MOODLE_INTEGRATION.md). CSP estricta en la UI.
- Ninguna entrega sin confirmación explícita; "confirmada" solo con evidencia de Moodle.

## Sesión guardada (2026-11-07)
- Se guardan solo cookies de `*.juntadeandalucia.es` (Moodle y CAS), como JSON en el llavero del sistema (Secret Service). Nunca la contraseña. No van a ficheros en claro ni a los logs.
- Equivalen a una sesión abierta hasta 8 h de inactividad (límite del servidor). Se restauran únicamente en el WebView de AulaDesk. «Cerrar sesión y olvidarla» hace logout en Moodle, borra las cookies locales y el secreto; desactivar «Mantener la sesión» borra el secreto.
- Si el llavero no está disponible, la sesión simplemente no se guarda (no hay alternativa en claro).
- El canal de desarrollo (`AULADESK_DEV_CTL=1`) permite navegar/volcar/descargar en la sesión desde un fichero; está apagado por defecto y solo acepta rutas de Moodle. No activar fuera de pruebas.

## Base de datos cifrada y Familias (2026-11)
- La BD está cifrada con SQLCipher; la clave aleatoria vive en el llavero del sistema, nunca en disco ni en logs. No ejecutes un binario nuevo con `AULADESK_DATA_DIR` distinto sin saber que comparte esa entrada del llavero.
- Familias tiene su propio webview oculto, su almacén de cookies y su entrada de llavero, separados de Moodle. Navegación limitada a `*.juntadeandalucia.es` y un único comando remoto (`ingest_familias`), validado contra un contrato de datos; una extracción inválida se rechaza entera.
- Todo dato de Familias lleva `student_id` y cada comando valida el alumno contra la cuenta. Desvincular borra los datos de la cuenta.
- No se leen valores de la ficha del alumno (DNI, Seguridad Social). Las acciones de escritura exigen confirmación explícita y dejan registro.
