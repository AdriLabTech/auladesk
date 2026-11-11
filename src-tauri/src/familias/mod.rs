//! Integración «Familias» (portal de familias y alumnado de la Junta de Andalucía): WebView propio y oculto, extracción de DOM
//! con extractores versionados, persistencia por alumno. Independiente de Moodle: ningún fallo de aquí toca sus tablas ni su sesión.
//! Estado honesto: el pipeline (ventana, sesión, contrato, persistencia, avisos) está probado con datos simulados;
//! los extractores y las recetas de navegación dependen del DOM real del portal y se escriben tras el reconocimiento (ver INTEGRATION_RESEARCH.md).
pub mod actions;
pub mod commands;
pub mod mapper;
pub mod recipes;
pub mod sync;
pub mod window;
