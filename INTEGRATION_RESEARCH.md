# INTEGRATION_RESEARCH — iPasen e iSéneca

Fecha: 2026-11-09. Investigación **preliminar con búsquedas web**; no se ha entrado en ningún portal ni se ha inspeccionado tráfico. Lo no confirmado se marca **(no verificado)**. No se usa ni se propone ingeniería inversa de las apps móviles ni saltarse autenticación.

## Hallazgo común
- No se ha encontrado **ninguna API pública, documentada ni para terceros** de iPasen/PASEN ni de Séneca. Una búsqueda sobre portales de API de la Junta solo devolvió la de la Consejería de Agricultura (no relacionada) y el portal de datos abiertos, sin dataset educativo individual. Ausencia de resultado ≠ prueba de que no exista: **confirmarlo preguntando a la Consejería de Desarrollo Educativo y FP** (pendiente).
- Lo que ya sabemos por Moodle (verificado en el repo): el SSO de Moodle Centros usa el CAS `edea.juntadeandalucia.es` (IdEA). Los manuales de centros indican que las **credenciales IdEA** son también las de PASEN/iPasen. Es probable que una sola sesión del WebView sirva para varios portales, **(no verificado: no he abierto PASEN con la sesión)**.

## iPasen / PASEN
- **Qué es**: app y web para familias y alumnado ≥ cierta edad (iOS/Android; la web de PASEN vive en el portal de Séneca según fuente no oficial). Fuente: [ficha App Store](https://apps.apple.com/app/id511127024) (versión 12.8.2, marzo 2025).
- **Funcionalidades documentadas por fuentes oficiales/centros**: calificaciones, faltas y retrasos con justificación (selección de falta + motivo), observaciones del tutor/profesorado, comunicaciones, solicitud y aceptación de tutorías ([guía rápida](https://blogsaverroes.juntadeandalucia.es/iescasabermeja/files/2021/04/GUÍA-RÁPIDA-DE-iPASEN.pdf), [manual 2024](https://blogsaverroes.juntadeandalucia.es/iesriberadelfardes/files/2024/06/manual_Ipasen.pdf)).
- **Acceso/autenticación**: credenciales IdEA ([manual familias](https://blogsaverroes.juntadeandalucia.es/ceipaznaitin/files/2020/11/CREDENCIALES-IdEA-EN-PASEN_iPASEN.-Manual-para-alumnado-y-familias.pdf)); «autologin» para tutores legales y alumnado con NIF para recuperar acceso. **Tras 2024 puede haber cambios (certificado/Cl@ve): (no verificado).**
- **Endpoints documentados**: ninguno encontrado.
- **Viabilidad**: **media-baja**, solo por WebView con la sesión del propio usuario (opción 4 de tu lista), extracción de DOM igual que Moodle. Condiciones: (a) que exista interfaz web utilizable en escritorio, (b) que el login por IdEA funcione en WebKitGTK, (c) que los términos lo permitan. Lectura únicamente; justificar faltas sería escritura → fuera de alcance inicial.
- **Restricciones**: datos de menores; varios alumnos por cuenta familiar → modelo `student` y selector de alumno activo, sin mezclar datos.
- **Alternativas**: abrir PASEN en el WebView embebido como «visor» (sin extracción) y sacar solo avisos básicos; o notificaciones por correo de PASEN (si el centro las envía, **no verificado**) leídas por el usuario.

## iSéneca / Séneca
- **Qué es**: sistema de información de gestión de centros (profesorado y administración), con app iSéneca. No aparece documentación pública técnica en los resultados.
- **Autenticación**: no confirmada. Otras comunidades usan certificado digital/Cl@ve junto a usuario/contraseña ([ejemplo EducamosCLM](https://ceip-parquedelamuneca.centros.castillalamancha.es/sites/ceip-parquedelamuneca.centros.castillalamancha.es/files/descargas/guia_acceso_educamosclm_a_traves_de_clave.pdf)); para Séneca **(no verificado)**. Un acceso con certificado hardware impediría un WebView automatizado.
- **Endpoints documentados**: ninguno.
- **Funcionalidades deseadas** (horarios, grupos, cuaderno, evaluación): no hay evidencia pública de cómo acceder programáticamente. Los datos son del alumnado de terceros: **máxima cautela**.
- **Viabilidad**: **baja** a corto plazo. Es también un perfil que el usuario actual (alumno DAM) **no tiene**, por lo que no se puede probar ni verificar nada real. Cualquier implementación sería a ciegas.
- **Recomendación**: no implementar iSéneca hasta tener (1) un usuario real con perfil docente que autorice y pruebe, y (2) confirmación de que el acceso está permitido. Mientras, dejar el contrato (`IntegrationStatus = NotAvailable`) y como mucho un acceso directo al portal en WebView.

## Autorización de la Junta
Sin información pública sobre programas de acceso para desarrolladores. Vía: escrito a la Consejería / centro; sede electrónica. **Pendiente** de que Adrián decida si lo plantea.

## Resultado del spike PASEN (2026-11-09)
Instancia de depuración aparte (datos en scratchpad), con la sesión de Moodle restaurada desde el llavero («Active») y las órdenes dev `navext` / `clicktext`.
- 10:36: `www.juntadeandalucia.es/educacion/portalseneca/web/pasen/inicio` mostraba «Servicio no disponible temporalmente (mantenimiento)»; a las 10:42 seguía igual desde el WebView, aunque Adrián ya accedía desde su navegador.
- **El acceso real está en `seneca.juntadeandalucia.es`** (portal público `/seneca/jsp/portal/`, mosaico con «PASEN Familias – versión para móviles y tabletas», «Acceso Séneca Formación», CAUCE…). Tras iniciar sesión las URL son de tipo `/seneca/nav/<token>`: **token ligado a la sesión del navegador, sin valor para otro cliente y sensible: no se guarda ni se reutiliza.**
- **Séneca NO comparte la sesión de Moodle/CAS.** «Acceso a Séneca» muestra su propio formulario: Usuario + Clave, botón QR, «¿Olvidó su contraseña?» y «Acceso con Certificado». Con la sesión del WebView de AulaDesk no se entra automáticamente.
- Consecuencias para el diseño: (1) PASEN/iSéneca necesitan **cuenta y almacén de cookies propios** (confirma la propuesta `linked_account`); (2) AulaDesk no debe manejar la clave: el login lo hace el usuario en el WebView, como con Moodle; (3) el acceso por QR/certificado dependerá de WebKitGTK y no es automatizable; (4) la misma entrada sirve para familias y docentes, así que el perfil se decide tras entrar.
- No visto aún (requiere que el usuario inicie sesión en el WebView): estructura de PASEN tras el login, selector de alumno, DOM de calificaciones/faltas, caducidad de sesión.
- No se leyó ningún dato personal. No se introdujo ninguna credencial.

## Qué falta para pasar de «preliminar» a «verificado»
1. Spike **de solo lectura y con la cuenta de Adrián**: abrir PASEN en el WebView existente con su sesión IdEA, ver si hay sesión compartida y si el DOM es extraíble. Necesito su OK porque implica navegar por datos personales.
2. Revisar términos de uso y política de privacidad de PASEN/Séneca.
3. Confirmar con la Consejería la existencia de APIs.

Fuentes: [App Store iPasen](https://apps.apple.com/app/id511127024), [guía rápida iPasen](https://blogsaverroes.juntadeandalucia.es/iescasabermeja/files/2021/04/GUÍA-RÁPIDA-DE-iPASEN.pdf), [manual iPasen 2024](https://blogsaverroes.juntadeandalucia.es/iesriberadelfardes/files/2024/06/manual_Ipasen.pdf), [credenciales IdEA](https://blogsaverroes.juntadeandalucia.es/ceipaznaitin/files/2020/11/CREDENCIALES-IdEA-EN-PASEN_iPASEN.-Manual-para-alumnado-y-familias.pdf), [datos abiertos Junta](https://juntadeandalucia.es/datosabiertos/portal/).


## Spike con webview propio (2026-11-11) — portal público, sin login
Resultado de abrir el portal en el WebView **de Familias** (almacén propio, oculto) y pedir el reconocimiento estructural (`frecon`), sin credenciales ni datos personales:
- `https://seneca.juntadeandalucia.es/seneca/jsp/portal/` carga y el puente se inyecta; el IPC remoto (capability por origen) funciona.
- **El portal es una SPA Angular** (`app-root` › `app-main` › `app-header` / `main` / `app-footer`, componentes `app-*`, Bootstrap, sin iframes). Consecuencias: (1) la navegación interna NO dispara cargas de página, así que las recetas esperan por tiempo tras cada pulsación (`wait_ms`, pausa por paso); (2) el reconocimiento debe hacerse con la página asentada (a los ~3 s tras cargar el `main` aún estaba vacío; a los ~9 s ya estaba pintado, 495 nodos); (3) conviene que los extractores esperen a que aparezca su contenedor antes de leer.
- Sin sesión la página se clasifica como `LoggedOut` (ruta `/seneca/jsp/portal`, sin `/seneca/nav/`).
- **Sigue sin verificar** todo lo posterior al login: criterio de sesión activa (`/seneca/nav/<token>` es lo observado el 9-nov), selector de alumno, estructura de calificaciones/faltas/comunicaciones, caducidad, y si el acceso con QR/certificado funciona dentro de WebKitGTK. Cl@ve u otras pasarelas fuera de `*.juntadeandalucia.es` están bloqueadas por la lista de navegación.
- Cómo continuar: Adrián inicia sesión en la ventana de Familias (Conexión › Abrir el portal) y, en cada pantalla, usa «Guardar esqueleto de la página actual»; con esos ficheros (texto sustituido por su forma) se escriben los extractores y las recetas (`familias_recipes.json`, también admite un `familias_recipes.json` local en la carpeta de datos para iterar sin recompilar).
