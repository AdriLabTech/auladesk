# Cambios

## 0.3.0 — 2026-11-11 — integración «Familias» (PASEN/iPasen) y rendimiento
- **Cuentas vinculadas** y alumnado (`linked_account`, `student`) y contrato común de integraciones (`integration.rs`). Moodle no cambia de comportamiento.
- **Familias**: webview propio y oculto con sesión y llavero separados; contrato de datos validado; calificaciones, faltas/retrasos, comunicaciones y tutorías por alumno; selector de alumno activo; avisos con origen y deduplicados; límites de frecuencia (auto 60 min, manual 5 min) y pausa; desvincular borra los datos de la cuenta.
- **Acciones de escritura** (justificar falta, pedir tutoría, mensaje) con confirmación explícita y registro. Sin receta verificada contra el portal quedan «no disponibles»: no se simula el envío.
- **Calendario y avisos unificados**: `calendar_event` admite varias fuentes (migración 0008), filtros por plataforma, insignia de origen. Dashboard con bloque por alumno y aviso de «sin sesión».
- **Migraciones 0007–0011** con copia previa automática en `exports/`.
- **Estado honesto**: los extractores y recetas del portal NO existen todavía (hace falta el reconocimiento con una cuenta real); todo lo demás está probado con datos simulados. Docentes (iSéneca) aparcado.
- Documentación: `docs/ARCHITECTURE.md` reescrita.
- **Extractores reales** de Familias (alumno, faltas, mensajes, notas) y sesión persistente (entrada por `Acceso.jsp`). **Sincronizar todo**: Moodle y Familias a la vez.

- **Rendimiento de Moodle** (medido con la cuenta real, 53 páginas): lectura SIN renderizar (`fetch` del HTML desde el puente y extracción del documento analizado; cae a la navegación clásica si falla) 104–117 s → 72–76 s. Paralelizar no sirve: Moodle atiende una petición por sesión (9 peticiones = 11,4 s con 1, 2, 3, 5 o 9 a la vez), por eso el pool de ventanas de lectura queda desactivado (`WORKERS = 0`). **Refresco por niveles**: solo se releen las tareas/cuestionarios que pueden haber cambiado (abiertas, sin fecha, cerradas hace <14 días; las antiguas cada 7 días). Botón «Sincronización completa» para releerlo todo. Un fallo de página se reintenta una vez.
- **Temas**: de 8 a 18. Siete nuevos (Violeta, Coral, Rubí, Mono, Sakura, Arena, Tinta) y tres **animados** (Atardecer, Marea, Espectro) cuyo acento cambia solo con una transición suave de CSS (sin bucle de animación; se quedan quietos con el modo tranquilo o al cambiar de color de acento). Test de contraste WCAG AA para todos (`tests/themes.test.ts`): corrigió «Papel» (4,2:1) y «Violeta».
- **Efectos decorativos** opcionales: ecualizador en la barra lateral u onda bajo la cabecera. No reaccionan al audio, no reciben el ratón y desaparecen con el modo tranquilo.
- **Accesibilidad**: texto secundario ≥ 4,5:1 en todos los temas, alto contraste (ajuste o `prefers-contrast`), `readableOn` por contraste real, foco atrapado y devuelto en los diálogos, enlace «Saltar al contenido», anuncio del cambio de sección y `lang` del documento.
- **Idioma**: español/inglés/automático. Cubre la estructura de la app, ajustes, paleta, loader, formatos de fecha y etiquetas comunes; el resto de vistas sigue en español hasta que se traduzca (nunca se rompe: lo que falta se muestra en español).
- **Ecualizador funcional**: mide el sonido que sale por la salida de audio por defecto (`parec` sobre el monitor de PulseAudio/PipeWire, FFT propia en Rust, 16 bandas, ~47 fotogramas/s) y mueve las barras del ecualizador y la onda. El audio solo se reduce a 16 números y se descarta (ni disco ni red); solo corre con el efecto activo, sin modo tranquilo y con la ventana visible. Solo Linux; sin `parec` o en otros sistemas se usa una animación decorativa. Tests de la FFT/bandas con tonos de 100 Hz a 9 kHz.
- **Pantallas de bienvenida y de carga**: cinco estilos (Círculo, Tipográfica, Terminal, Cuadrícula, Minimalista) y «Aleatoria»; el telón de arranque cambia a juego.
- **Traducción ES/EN de toda la interfaz**: ~800 cadenas, test que exige traducción de cada `t()`; avisos y notificaciones del backend (`i18n.rs`, en el único punto `notif::add`, para bandeja, escritorio y servicio) y errores del backend (`en_errors.ts`, en el envoltorio de `invoke`). Lo ya guardado no se retraduce.
- **Configuración a ancho completo** en cuadrícula de tarjetas.
- **Sesión de Familias al arrancar**: una sesión guardada que el servidor ya no acepta (el portal devuelve una página de error) se detecta, se descarta y el estado pasa a «Sesión caducada» con aviso, en vez de quedarse en «Sin comprobar». Mientras la sesión está activa se mantiene viva (petición ligera cada 4 min, sin sincronización en curso) y se renueva la copia del llavero cada 5 min.
- **Sincronizar al iniciar** (opción en Moodle › Conexión y en Familias › Conexión, desactivada por defecto): al arrancar, si hay sesión guardada, se espera a que se restaure y se sincroniza Moodle y Familias a la vez. Mientras dura se muestra la pantalla de espera (tras el saludo de bienvenida, con el tiempo transcurrido) y al terminar se recarga la vista. Sin sesión guardada la plataforma se omite al instante; Familias respeta el límite de 5 min entre sincronizaciones manuales. Medido: arranque completo en ~57 s (Moodle 21 s con refresco por niveles, Familias ~54 s, en paralelo).
- **Loader bloqueante** durante sincronizaciones y descargas: el propio loader (cuadrados recorriendo un cuadrado) es una serpiente que se dirige con flechas/deslizando; sin puntuaciones; se cierra al terminar aunque haya partida. Ajuste para desactivarlo.
- **Personalización**: densidad (compacta/normal/amplia) y tamaño de la interfaz (90–125 %).
- Saludo de bienvenida: un telón de arranque evita ver el Dashboard antes del saludo.
- Se elimina el módulo «Docentes» (iSéneca, no disponible): vista, sección, integración y cuenta.

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
