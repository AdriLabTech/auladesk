# Base de datos cifrada (SQLCipher)

Estado (2026-11-09, rama `integraciones-educativas`): implementado y probado en Linux; **sin verificar en Windows/macOS ni con la app real instalada**.

## Qué se hizo
- `Cargo.toml`: `sqlx` con `sqlite-unbundled` + `libsqlite3-sys 0.30` con `bundled-sqlcipher-vendored-openssl` (SQLCipher 4.5.7 *community* + OpenSSL compilados dentro; sin librerías del sistema).
- `db::open_app` (usado por la app y por `auladesk --notifier`): clave aleatoria de 256 bits guardada en el llavero (servicio `moodledesk`, usuario `database-key`).
  - BD nueva → nace cifrada. BD en claro → se cifra con `encrypt_in_place`.
  - `encrypt_in_place`: exporta a `*.db.cifrando`, compara nº de tablas y filas con la clave, y solo entonces la sustituye. **El original no se borra**: queda como `moodledesk.db.sin-cifrar`; bórrala tú (mejor con `shred`) cuando compruebes que todo funciona.
  - Sin llavero: BD en claro → sigue en claro con aviso en el log; BD ya cifrada → error claro y no se toca nada.
- `db::open` (en claro) se conserva para los tests; las copias de seguridad (`VACUUM INTO`) de una BD cifrada salen cifradas (test).

## Verificado
- Experimento aparte con las mismas versiones: cifra, rechaza clave incorrecta y ausente, sin texto legible en el fichero.
- Tests nuevos en `db.rs` (cifrar conservando datos y migraciones, copia cifrada, claves). Suite de esta rama: 68 OK, 2 ignorados.
- Test manual sobre una **copia** de tu BD real: 25 tablas, 6 migraciones, cabecera ilegible.
- Arranque en modo `--notifier` con carpeta de datos temporal: crea la BD cifrada y la vuelve a abrir con la clave del llavero.

## Riesgos y reglas
- **Perder la clave = perder la BD.** Si se borra el llavero (reinstalación, otro usuario) los datos no se pueden recuperar; la fuente de verdad siguen siendo Moodle/PASEN y los archivos en disco (que no están cifrados: solo la BD). Las copias de seguridad cifradas necesitan la misma clave.
- **El archivador/servicio antiguo** (`auladesk --notifier` instalado) no sabe leer la BD cifrada: hay que actualizar el binario del servicio a la vez que la app (`service_install`).
- **Los ficheros descargados** (biblioteca, entregas, caché) y los logs **no** se cifran con esto. Si se quiere proteger también eso, ver cifrado de disco del sistema.
- **Migración 0006**: contiene las entregas programadas (en uso) y dos tablas de Drive que ya no se usan; se conservan porque las migraciones aplicadas no se editan.
- Windows/macOS: `vendored-openssl` necesita Perl y compilador C en CI (`release.yml`); **no probado**.
- CI y Docker: `libssl-dev` ya está instalado; falta comprobar tiempo de compilación y `perl` en la imagen.
