# Riesgos legales y publicación — AulaDesk

Fecha: 2026-11-09. **Esto es un análisis técnico-práctico, no asesoramiento jurídico.** Lo marcado *(no verificado)* no se ha podido confirmar en fuentes oficiales; antes de una publicación amplia o comercial conviene una consulta breve a un abogado de propiedad intelectual / protección de datos.

Fuentes consultadas: [Política de marca de Moodle (TRADEMARK.txt)](https://raw.githubusercontent.com/moodle/moodle/main/TRADEMARK.txt), [Aviso legal de la Junta de Andalucía](https://juntadeandalucia.es/informacion/legal.html), [licencia de SQLCipher (BSD-3)](https://raw.githubusercontent.com/sqlcipher/sqlcipher/master/LICENSE.md).

## 1. Resumen
Publicarlo como proyecto **independiente, gratuito, de código abierto, local y sin servidores** es defendible con riesgo bajo **si se cumplen las medidas de la sección 8**. El riesgo sube si se **cobra**, se usa el **nombre/logotipos** de terceros, se incluyen **datos reales** en capturas, o se **sube a la nube** información de PASEN.

## 2. Marcas
| Elemento | Qué dice la fuente | Riesgo | Solución |
|---|---|---|---|
| «Moodle» | Prohibido sin permiso escrito usarlo en el **nombre del software (incluidas apps)**, empresa o dominio, y en uso comercial; **permitido** el uso descriptivo/nominativo: describir software propio que **se integra con** Moodle. Contacto de dudas: `trademarks@moodle.com`. | Bajo con el nombre actual (**AulaDesk**, no lleva «Moodle»). Medio por restos: carpeta/repo/BD llamados `moodledesk` y el servicio de llavero `moodledesk`. | Nombre público = AulaDesk. Usar «Moodle» solo para describir la integración («compatible con Moodle»). No usar logotipos de Moodle. Mantener el aviso de no afiliación (ya está en el README). Renombrar el repo/carpeta si se publica con ese nombre; los identificadores internos (BD, llavero) pueden quedar por compatibilidad (no son marca visible). Opcional: escribir a `trademarks@moodle.com` para confirmar. |
| «PASEN», «iPasen», «Séneca» | El aviso legal de la Junta prohíbe reproducir **logotipos, escudos, símbolos y marcas identificativas** sin autorización expresa y exige no sugerir que la Junta apoya el uso. No he podido comprobar si «PASEN/iPasen/Séneca» están registrados como marca en la OEPM *(no verificado)*. | Medio si se usan como nombre o logotipo; bajo si solo se mencionan de forma descriptiva. | **Renombrar la sección en la interfaz** (decisión de Adrián): **«Familias»** (portal de familias del centro) en lugar de PASEN/iPasen. El módulo «Docentes» (iSéneca) se eliminó del código el 2026-11-11. Mencionar PASEN/Séneca solo en la documentación, como texto descriptivo y con aviso de no afiliación. No usar logos, colores corporativos ni capturas del portal con su cabecera. Consultar el localizador de marcas de la OEPM antes de publicar. |
| Logotipo/escudo de la Junta | Prohibido sin autorización | Alto si aparece | No incluir ninguno (ni en iconos, ni capturas, ni web). Comprobar que `docs/img/*` y `src-tauri/icons/*` no los contienen. |

## 3. Derechos de autor y datos de terceros
- **Código y diseño de Moodle**: AulaDesk no incluye código de Moodle (GPL); lee páginas ya servidas al usuario. Los **selectores** (`bridge.js`) son código propio. No copiar plantillas, CSS ni textos de Moodle.
- **Contenido de los cursos** (archivos del profesorado): el usuario lo descarga para sí, igual que desde el navegador; AulaDesk no lo redistribuye. **No debe subirse a Drive público ni compartirse**; la función de Drive sube solo a la cuenta del propio usuario (documentarlo).
- **Textos de la Junta**: el aviso los publica con CC-BY 3.0 (reconocimiento: «Información obtenida del Portal de la Junta de Andalucía»). Si se copian textos de ayuda, citar así. Preferible escribir los propios.
- **Séneca declarada software de fuente abierta** (convenio publicado en el BOE, 2021 *(no verificado el estado actual del código)*): no copiar su código ni sus recursos; no se necesita.
- **Capturas y vídeos**: solo con datos simulados (como ya se hace). **Nunca** capturas reales de PASEN (menores).

## 4. Acceso automatizado a los portales
- El aviso legal de la Junta **no menciona** scraping ni bots; prohíbe **sobrecargar o inutilizar** los servicios, interferir con otros y **acceder sin autorización** (hacking).
- AulaDesk usa la **sesión del propio usuario**, la misma que en el navegador, **solo lectura**, sin saltarse autenticación (login interactivo, sin guardar contraseña, sin eludir captchas/MFA). No se hace ingeniería inversa de las apps móviles ni de APIs privadas.
- Límites de buena conducta que deben quedar **en el código** (no solo en este documento): una sincronización de PASEN como máximo cada 60 min, sin paralelismo, reintentos con espera, parada al primer error 4xx/5xx repetido y un interruptor «pausar sincronización».
- Riesgo residual: los **términos de uso de Séneca/PASEN** (no localizados) podrían prohibir clientes no oficiales; la Junta podría bloquear o cambiar la web *(no verificado)*. Mitigación: comunicación previa al **Servicio de atención / Delegación de Protección de Datos** de la Consejería, ofreciendo el código; texto del README que no prometa disponibilidad.
- Derecho «sui generis» de bases de datos (TRLPI): extraer para uso personal una parte no sustancial es el uso normal de un usuario; no se ofrece ningún servicio de volcado masivo ni agregación entre usuarios. *(criterio general, no verificado con jurisprudencia)*.

**Implementado (2026-11-11):** `familias::commands` fuerza una sincronización automática como máximo cada 60 min y una manual cada 5, sin paralelismo y con pausa entre pasos, y hay interruptor «pausar». Pendiente de implementar: parada tras errores 4xx/5xx repetidos y reintentos con espera creciente (hoy un fallo cuenta como intento y espera el intervalo normal).

## 5. Protección de datos (RGPD / LOPDGDD)
- Con arquitectura **local, sin servidor del autor, sin telemetría ni informes de errores**, el autor **no trata** datos personales: no es responsable ni encargado. El usuario (familia) trata los datos de sus hijos para fines domésticos. **Mantener esa propiedad es el punto crítico**: cualquier telemetría, nube propia o servicio de soporte con acceso a datos cambiaría el análisis.
- **Datos de menores (PASEN)**: cifrados en reposo con SQLCipher (ver `docs/SQLCIPHER.md`), clave en el llavero del sistema, sin logs con contenido, aislamiento por alumno.
- **Drive**: la copia a Google Drive es opcional, solo de **archivos del propio usuario de Moodle**; **excluir por diseño** datos de PASEN de cualquier exportación a terceros.
- **Perfil docente (iSéneca)**: el tratamiento de datos de alumnado ajeno convierte al docente/centro en responsable con obligaciones propias; es una razón más para mantenerlo «no disponible» hasta contar con autorización de la Consejería.
- Publicar una `PRIVACY.md` clara: qué se guarda, dónde, cómo borrarlo (desconectar cuenta = borrar cookies, llavero y datos de esa cuenta).

## 6. Licencias de dependencias (publicar el binario obliga a avisar)
| Componente | Licencia | Obligación |
|---|---|---|
| SQLCipher (Community) | BSD-3 (Zetetic) | Reproducir el aviso, condiciones y descargo en la documentación/materiales de la distribución; no usar el nombre de Zetetic para promocionar. |
| OpenSSL 3 (vendored) | Apache-2.0 *(no verificado en este repo)* | Incluir licencia/NOTICE. |
| Tauri, sqlx, tokio, etc. | MIT/Apache-2.0 *(no verificado uno a uno)* | Incluir avisos. |
| pdf.js | Apache-2.0 | Incluir licencia/NOTICE (ya citado en PROGRESS). |
| Tipografías (Space Grotesk, Inter, JetBrains Mono, Fraunces) | SIL OFL *(no verificado el origen de los ficheros)* | Incluir licencia; no vender la fuente sola. |
| AulaDesk | MIT (LICENSE) | Compatible con lo anterior. |

Acción: generar `THIRD_PARTY_NOTICES.md` con `cargo about` / `cargo deny` y `license-checker` para npm, e incluirlo en los instaladores y en la pantalla «Acerca de».

## 6 bis. Criptografía y exportación
SQLCipher/OpenSSL son criptografía de uso general de código abierto; publicar el código fuente y binarios gratuitos suele estar exento o con notificación simple en EE. UU./UE *(no verificado)*. Sin riesgo práctico para un proyecto personal; relevante solo si se vende.

## 7. Qué NO hacer (líneas rojas)
1. No guardar contraseñas ni claves de familias/docentes (decisión ya tomada: sesión guardada).
2. No usar «Moodle», «PASEN», «iPasen», «Séneca», «Junta de Andalucía» en el nombre del producto, dominio, icono o marketing.
3. No publicar capturas, vídeos ni BD con datos reales.
4. No saltarse login, captcha, MFA ni límites de tasa.
5. No ofrecer el servicio como alojado (servidor propio que guarde datos de terceros).
6. No cobrar ni aceptar donaciones asociadas al acceso a PASEN/Séneca sin revisar de nuevo este documento.
7. No implementar escritura (justificar faltas, calificar) sin autorización expresa.

## 8. Lista de comprobación antes de publicar
- [ ] Nombre y textos de la UI: «Familias»; PASEN/Séneca solo en docs, con aviso de no afiliación.
- [ ] Sin logos ni colores corporativos de la Junta/Moodle; revisar `docs/img`, iconos y README.
- [ ] README con aviso: independiente, no afiliado, uso bajo responsabilidad del usuario, solo con tus propias credenciales y acceso legítimo.
- [ ] `PRIVACY.md` + `THIRD_PARTY_NOTICES.md` + aviso en «Acerca de».
- [ ] Límites de frecuencia implementados y con test.
- [ ] Capturas solo con datos simulados.
- [ ] BD cifrada por defecto (hecho en esta rama) y sin copias en claro olvidadas (`*.db.sin-cifrar`).
- [ ] Consulta OEPM de «PASEN», «iPasen», «Séneca», «AulaDesk» (esta última por colisión de nombre).
- [ ] Opcional pero recomendable: correo a `trademarks@moodle.com` y comunicación a la Consejería (Servicio de atención / DPD) con enlace al código.
- [ ] Consulta legal puntual si se piensa cobrar o distribuir fuera de un uso personal/educativo.
