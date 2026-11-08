# AulaDesk

Cliente de escritorio, **personal y de solo lectura**, para plataformas [Moodle](https://moodle.org): reúne tus tareas, fechas límite, archivos y avisos en una aplicación local rápida, y te ayuda a **preparar y comprobar entregas sin enviar nada por ti**.

> AulaDesk es un proyecto independiente. **No está afiliado, patrocinado ni respaldado por Moodle Pty Ltd, Moodle HQ ni la Junta de Andalucía.** «Moodle» es una marca de sus titulares.

![Dashboard](docs/img/dashboard.png)

*Capturas con datos de ejemplo (nombre y tareas ficticios).*

## Qué hace

- **Dashboard**: saludo con resumen real, progreso, prioridades con puntuación transparente, próximos 14 días y actividad reciente.
- **Asignaturas y biblioteca**: sincroniza cursos, temas y actividades; descarga archivos con verificación SHA-256, sin duplicados y con versiones.
- **Tareas**: estado y fecha límite de cada tarea en **lista o tablero** (por estado, asignatura o prioridad, con arrastre en este último).
- **Entregas**: copias versionadas e inmutables de lo que preparas. Una entrega solo figura como *confirmada* si Moodle muestra un archivo con el **mismo SHA-256**. AulaDesk **nunca envía nada**: abre la página oficial y adjuntas y envías tú.
- **Calendario y avisos**: eventos derivados de las fechas reales, recordatorios configurables (también con la app cerrada mediante un servicio de usuario opcional).
- **Apariencia**: 8 temas predefinidos, acento propio, esquinas y modo tranquilo; paleta de comandos `Ctrl+K`.

![Tablero](docs/img/tablero.png)
![Temas](docs/img/temas.png)

## Principios de seguridad y privacidad

- **Nunca guarda tu contraseña.** Inicias sesión tú en la ventana de Moodle (incluido el SSO de tu centro).
- La sesión se conserva como cookies de los dominios de la plataforma en el **llavero del sistema** (Secret Service); caduca según el servidor. «Cerrar sesión y olvidarla» hace logout y borra todo.
- Todo se guarda **en local** (SQLite + carpeta de archivos). Sin telemetría ni servicios de terceros.
- Solo lee las páginas que ya ves con tu sesión; **no automatiza envíos** ni usa tokens ajenos. Detalle en [`docs/SECURITY.md`](docs/SECURITY.md).
- Usa la web de Moodle como cualquier navegador; comprueba que las normas de uso de tu centro permiten herramientas de este tipo antes de usarla.

## Estado y límites

Proyecto personal en desarrollo (`v0.1`), **desarrollado y probado contra una instancia concreta** (Moodle 5.x de la Junta de Andalucía, tema «moove»): la URL base y los selectores están fijados en ese entorno (`src-tauri/src/moodle/`, `src-tauri/selectors/bridge.js`). Adaptarlo a otra instalación exige tocar esas constantes y revalidar los selectores. Lo verificado y lo **no** verificado está listado con honestidad en [`PROGRESS.md`](PROGRESS.md).

Probado en Linux (Arch, Hyprland/Wayland). Windows y macOS no se han probado.

## Compilar y ejecutar

Requisitos: Rust estable, Node.js 20+, dependencias de Tauri 2 para Linux (WebKitGTK 4.1, libsecret) y un servicio de llavero (gnome-keyring o similar).

```bash
npm install
npm run check                      # tipos de Svelte
(cd src-tauri && cargo test --lib) # 61 pruebas
npx tauri build --debug --no-bundle
./src-tauri/target/debug/auladesk
```

Variables de desarrollo: `AULADESK_DATA_DIR` (carpeta de datos alternativa, útil para probar sin tocar tus datos) y `AULADESK_DEV_CTL=1` (canal de control local solo para desarrollo, apagado por defecto).

> Nota: por compatibilidad con instalaciones previas, la carpeta de datos (`~/.local/share/moodledesk`), el identificador de Tauri y la entrada del llavero conservan el nombre histórico del proyecto.

## Arquitectura

Tauri 2 (Rust) + Svelte 5 + SQLite. Resumen en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); base de datos en [`docs/DATABASE.md`](docs/DATABASE.md); integración con Moodle en [`docs/MOODLE_INTEGRATION.md`](docs/MOODLE_INTEGRATION.md).

## Licencia

[MIT](LICENSE). Las tipografías (Inter, Space Grotesk, Fraunces, JetBrains Mono) **no se incluyen**: se usan si están instaladas en el sistema.
