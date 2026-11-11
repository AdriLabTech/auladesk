// Puente del portal de Familias. Solo LEE el DOM de la página que el usuario ya ve; no escribe nada en el portal.
// Estado: infraestructura (sesión, reconocimiento, pulsaciones guiadas) + registro de extractores VACÍO:
// los extractores se escriben contra el DOM real tras `recon` con la cuenta de la familia. Sin extractor → error explícito, nunca datos inventados.
(() => {
  if (window.__fmBridge) return;
  const VERSION = 'f1';
  window.__fmBridge = VERSION;

  const send = (kind, payload) => {
    const inv = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
    if (!inv) return Promise.resolve();
    return inv('ingest_familias', { kind, url: location.href, payload }).catch((e) => {
      try { document.title = '[fmbridge error-' + kind + ': ' + String(e).slice(0, 300) + '] ' + document.title; } catch (_) {}
    });
  };
  const text = (el) => (el ? el.textContent.replace(/\s+/g, ' ').trim() : '');
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const fail = (kind, why) => send('error', { kind, selector_version: VERSION, message: why });
  const visible = (el) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none'; };

  // ── Utilidades de lectura (portal Séneca: formularios `select`, tablas `table-hover` con celdas `snc-col-N`).
  const norm = (x) => String(x == null ? '' : x).replace(/\s+/g, ' ').trim();
  const lower = (x) => norm(x).toLowerCase();
  const sha = async (x) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(x)))].map((b) => b.toString(16).padStart(2, '0')).join('');
  const waitFor = async (pred, ms = 15000) => { const t0 = Date.now(); while (Date.now() - t0 < ms) { const v = pred(); if (v) return v; await sleep(250); } return null; };
  const body = () => document.getElementById('contenedorCuerpo');
  const noData = () => { const c = body() && body().querySelector('.info-card'); return !!c && /no existen datos/i.test(c.textContent); };
  const dataTable = () => [...(body() ? body().querySelectorAll('table') : [])].find((t) => t.querySelector('tbody tr') && !/Cloned/.test(t.id));
  const heads = (t) => [...t.querySelectorAll('thead th')].map((h) => lower(h.textContent));
  const col = (tr, n) => { const td = tr.querySelector('td.snc-col-' + n); return td ? norm(td.textContent) : ''; };
  const h1 = () => lower((document.querySelector('h1.title-md, .content-header h1') || {}).textContent);
  // Perfil activo del portal: curso y centro de la barra lateral. La huella de ambos identifica al alumno en AulaDesk (el portal solo muestra un perfil a la vez).
  const profile = () => { const b = document.querySelector('#perfiles-alumnos-btn .datos-centro'); if (!b) return null; const course = norm((b.querySelector('strong') || {}).textContent); const center = norm(norm(b.textContent).replace(course, '')); return course || center ? { course, center } : null; };
  const studentRef = async () => { const p = profile(); return p ? 'p:' + (await sha(p.course + '|' + p.center)).slice(0, 20) : null; };
  const dmyToDate = (x) => { const m = /^(\d{2})\/(\d{2})\/(\d{2,4})(?:\s+(\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(norm(x)); if (!m) return null; const y = Number(m[3]) < 100 ? 2000 + Number(m[3]) : Number(m[3]); return { y, mo: Number(m[2]), d: Number(m[1]), h: Number(m[4] || 0), mi: Number(m[5] || 0), s: Number(m[6] || 0) }; };
  const isoDay = (x) => { const d = dmyToDate(x); return d ? String(d.y).padStart(4, '0') + '-' + String(d.mo).padStart(2, '0') + '-' + String(d.d).padStart(2, '0') : null; };
  const epoch = (x) => { const d = dmyToDate(x); return d ? Math.floor(new Date(d.y, d.mo - 1, d.d, d.h, d.mi, d.s).getTime() / 1000) : null; };   // hora local del equipo (se asume la del centro)
  const uniqueIds = (items) => { const seen = new Map(); for (const it of items) { const n = (seen.get(it.id) || 0) + 1; seen.set(it.id, n); if (n > 1) it.id += '#' + n; } return items; };

  // ── Extractores (escritos contra el DOM real el 2026-11-11). Cada uno devuelve el contrato `f1` de `familias::mapper`.
  const EXTRACTORS = {
    // Alumno: nombre desde la «Ficha del alumno/a» (solo los campos de nombre y apellidos), curso y centro desde el perfil activo.
    students: async () => {
      if (!(await waitFor(() => document.getElementById('NOMBRE_ALUM')))) return fail('students', 'no se reconoce la ficha del alumno');
      const ref = await studentRef(); const p = profile();
      const name = norm(['NOMBRE_ALUM', 'APELLIDO1_ALUM', 'APELLIDO2_ALUM'].map((i) => (document.getElementById(i) || {}).value || '').join(' '));
      if (!ref || !name) return fail('students', 'perfil o nombre del alumno no encontrados');
      return send('students', { selector_version: VERSION, container_found: true, students: [{ ref, name, center: p.center || null, group: p.course || null }] });
    },
    // Faltas y retrasos: tabla Fecha · Tramo horario · Ausencia de · Estado (filtro «Todas»).
    attendance: async () => {
      const sel = await waitFor(() => document.getElementById('C_TIPFAL'));
      if (!sel) return fail('attendance', 'pantalla de faltas no reconocida');
      if (lower(sel.options[sel.selectedIndex].text) !== 'todas') return fail('attendance', 'el filtro de faltas no está en «Todas»');
      await waitFor(() => dataTable() || noData());
      const ref = await studentRef(); if (!ref) return fail('attendance', 'perfil del alumno no encontrado');
      const t = dataTable(); const items = [];
      if (t) {
        const h = heads(t); const ix = (re) => h.findIndex((x) => re.test(x)); const [iF, iT, iA, iE] = [ix(/^fecha/), ix(/tramo/), ix(/ausencia/), ix(/estado/)];
        if (iF < 0 || iE < 0) return fail('attendance', 'cabeceras de la tabla de faltas no reconocidas');
        for (const tr of t.querySelectorAll('tbody tr')) {
          const day = isoDay(col(tr, iF)); const estado = col(tr, iE); if (!day) return fail('attendance', 'fecha de falta no reconocida');
          const slot = iT >= 0 ? col(tr, iT) : ''; const subject = iA >= 0 ? col(tr, iA) : '';
          items.push({ id: 'f:' + (await sha([day, slot, subject].join('|'))).slice(0, 24), day, slot: slot || null, subject: subject || null, kind: /retraso/i.test(estado) ? 'retraso' : 'falta', justified: /^justificad/i.test(estado), justification: null });
        }
      } else if (!noData()) return fail('attendance', 'ni tabla de faltas ni aviso de «no existen datos»');
      return send('attendance', { selector_version: VERSION, container_found: true, student_ref: ref, items: uniqueIds(items) });
    },
    // Mensajes de entrada (Pasen): fecha · asunto · remitente · fecha de lectura (vacía = sin leer).
    messages: async () => {
      if (!(await waitFor(() => document.getElementById('RECIBIDO_COMO')))) return fail('messages', 'pantalla de mensajes no reconocida');
      if (!/mensajes de entrada/.test(h1())) return fail('messages', 'no es la bandeja de entrada');
      await waitFor(() => dataTable() || noData());
      const ref = await studentRef(); if (!ref) return fail('messages', 'perfil del alumno no encontrado');
      const t = dataTable(); const items = [];
      if (t) {
        for (const tr of t.querySelectorAll('tbody tr')) {
          const date = col(tr, 1), sender = col(tr, 6), readAt = col(tr, 7);
          const subject = col(tr, 5) || '(sin asunto)';   // hay mensajes sin asunto
          if (!date) continue;
          const sent = epoch(date); if (sent === null) return fail('messages', 'fila de mensaje no reconocida');
          items.push({ id: 'm:' + (await sha([date, sender, subject].join('|'))).slice(0, 24), subject, sender: sender || null, body: null, sent_at: sent, read: !!readAt });
        }
      } else if (!noData()) return fail('messages', 'ni tabla de mensajes ni aviso de «no existen datos»');
      return send('messages', { selector_version: VERSION, container_found: true, student_ref: ref, kind: 'mensaje', items: uniqueIds(items) });
    },
    // Notas de UNA evaluación (la elegida en el desplegable). Con datos, solo se reconoce una tabla con columnas de materia y calificación:
    // la estructura con notas reales NO se ha visto todavía (las evaluaciones consultadas estaban vacías).
    grades: async () => {
      const sel = await waitFor(() => document.getElementById('X_CONVCENTRO'));
      if (!sel) return fail('grades', 'pantalla de notas no reconocida');
      const evaluation = norm(sel.options[sel.selectedIndex].text);
      if (!evaluation) return fail('grades', 'no hay evaluación elegida');
      await waitFor(() => dataTable() || noData());
      const ref = await studentRef(); if (!ref) return fail('grades', 'perfil del alumno no encontrado');
      const t = dataTable(); const items = [];
      if (t) {
        const h = heads(t); const ix = (re) => h.findIndex((x) => re.test(x)); const [iS, iG] = [ix(/materia|asignatura/), ix(/calificaci|nota/)];
        if (iS < 0 || iG < 0) return fail('grades', 'tabla de notas con estructura aún no reconocida');
        for (const tr of t.querySelectorAll('tbody tr')) {
          const subject = col(tr, iS), grade = col(tr, iG); if (!subject && !grade) continue;
          if (!subject || !grade) return fail('grades', 'fila de notas no reconocida');
          const num = Number(grade.replace(',', '.')); items.push({ id: 'g:' + (await sha([evaluation, subject].join('|'))).slice(0, 24), subject, evaluation, grade_text: grade, grade_num: Number.isFinite(num) && num >= 0 && num <= 10 ? num : null });
        }
      } else if (!noData()) return fail('grades', 'ni tabla de notas ni aviso de «no existen datos»');
      return send('grades', { selector_version: VERSION, container_found: true, student_ref: ref, evaluation_scope: evaluation, items: uniqueIds(items) });
    },
  };

  // ── Sesión: sin contraseñas ni tokens. Solo si hay formulario de acceso y la forma de la ruta (los tokens de /seneca/nav/<token> no salen de aquí).
  const session = () => send('session', {
    host: location.host,
    path_kind: location.pathname.startsWith('/seneca/nav/') ? 'nav' : location.pathname.startsWith('/seneca/jsp/portal') ? 'portal' : 'other',
    has_password_field: !!document.querySelector('input[type="password"]'),
    // Página de error del portal (p. ej. al restaurar una sesión que el servidor ya no reconoce): no es el portal ni una sesión válida.
    error_page: !!document.getElementById('errorBodyContainer'),
  });

  // ── Reconocimiento: esqueleto estructural del DOM con el TEXTO sustituido por su «forma» (A mayúscula, a minúscula, 9 dígito),
  //    salvo las etiquetas de interfaz (th, label, button, legend, títulos). Se guarda solo en local para escribir selectores.
  const shape = (s) => s.replace(/\p{Lu}/gu, 'A').replace(/\p{Ll}/gu, 'a').replace(/\d/g, '9').replace(/(.)\1{2,}/g, '$1$1').slice(0, 24);
  const LITERAL = new Set(['TH', 'LABEL', 'BUTTON', 'LEGEND', 'H1', 'H2', 'H3', 'H4', 'CAPTION']);
  const maskPath = (h) => { try { return new URL(h, location.href).pathname.split('/').map((p) => (p.length > 24 ? '*' : p.replace(/\d/g, '9'))).join('/'); } catch (_) { return ''; } };
  // Menús y botoneras son texto de interfaz (sin datos personales): se conservan literales, salvo el bloque del perfil/alumno.
  const isUi = (el) => !!el.closest && !!el.closest('nav, #menuPrincipal-SNC, ul.nav, [role="tablist"], .nav-tabs, .breadcrumb, .content-header-botonera') && !el.closest('.unico-perfil, #perfiles-alumnos-btn');
  const recon = () => {
    let budget = 6000;
    const walk = (el, depth) => {
      if (budget-- <= 0 || depth > 14) return null;
      const n = { t: el.tagName.toLowerCase() };
      if (el.id) n.i = el.id.slice(0, 40);
      if (el.classList && el.classList.length) n.c = [...el.classList].slice(0, 4).map((c) => c.slice(0, 30));
      const ui = isUi(el);
      for (const a of ['role', 'type', 'name', 'aria-label', 'title', 'data-testid']) { const v = el.getAttribute && el.getAttribute(a); if (v) (n.a = n.a || {})[a] = (a === 'aria-label' || a === 'title') && !ui ? shape(v) : v.slice(0, 60); }
      if (el.tagName === 'A' && el.getAttribute('href')) n.h = maskPath(el.getAttribute('href'));
      const own = [...el.childNodes].filter((c) => c.nodeType === 3).map((c) => c.textContent).join(' ').replace(/\s+/g, ' ').trim();
      if (own) n.x = (LITERAL.has(el.tagName) || ui) ? own.slice(0, 60) : shape(own);
      if (!visible(el) && el !== document.body) n.hidden = true;
      const kids = [...el.children].filter((c) => !['SCRIPT', 'STYLE', 'NOSCRIPT', 'SVG', 'PATH'].includes(c.tagName.toUpperCase())).slice(0, 60).map((c) => walk(c, depth + 1)).filter(Boolean);
      if (kids.length) n.k = kids;
      return n;
    };
    send('recon', { selector_version: VERSION, path: maskPath(location.href), title: shape(document.title), iframes: document.querySelectorAll('iframe').length, tree: walk(document.body, 0) });
  };

  // ── Acciones guiadas por las recetas (lista cerrada).
  // Candidato más interno: si un <li> y su <a> contienen el texto, se pulsa el <a>. Primero coincidencia exacta, luego «contiene».
  const pick = (sel, want, exact) => {
    const c = [...document.querySelectorAll(sel)].filter((e) => visible(e) && (exact ? text(e).toLowerCase() === want : text(e).toLowerCase().includes(want)));
    return c.find((e) => !c.some((o) => o !== e && e.contains(o))) || null;
  };
  const MENU = 'a.nav-link, a.nav-item';
  // Si la opción está dentro de un grupo de menú plegado, se despliega ese grupo (solo si está plegado: pulsar un grupo abierto lo cierra).
  // Despliega TODOS los grupos plegados que contienen la opción, del más externo al más interno (p. ej. Comunicaciones › Mensajes › Mensajes de entrada).
  // Pulsar un grupo ya abierto lo cierra, así que solo se pulsan los plegados. ul#m-<N> ↔ a#h<N> (N puede ser negativo: ul#m--5 ↔ a#h-5).
  const unfold = async (want) => {
    const a = [...document.querySelectorAll('#menupasen ' + MENU)].find((e) => lower(e.textContent) === want);
    if (!a) return null;
    const chain = [];
    for (let u = a.closest('ul.collapse'); u; u = u.parentElement && u.parentElement.closest('ul.collapse')) chain.unshift(u);
    for (const ul of chain) {
      if (ul.classList.contains('show')) continue;
      const head = document.getElementById('h' + ul.id.slice(2));
      if (head) { head.click(); await waitFor(() => ul.classList.contains('show'), 3000); await sleep(300); }
    }
    return a;
  };
  const clickText = async (t) => {
    const want = String(t || '').trim().toLowerCase();
    const sel = 'a,button,[role="button"],[role="tab"],li,span.nav-link';
    let el = pick(sel, want, true) || pick(sel, want, false);
    if (!el) el = await unfold(want);
    if (!el) return fail('click', 'no se encontró «' + want.slice(0, 40) + '» en la página');
    el.click();
    return send('step', { ok: true });
  };
  // Elige una opción por su texto exacto en un desplegable y lanza su evento de cambio (el portal recarga los datos).
  const selectOption = async (id, option) => {
    const s = await waitFor(() => document.getElementById(String(id)));
    if (!s) return fail('select', 'no hay desplegable «' + String(id).slice(0, 30) + '»');
    const i = [...s.options].findIndex((o) => norm(o.text) === norm(option));
    if (i < 0) return fail('select', 'el desplegable no tiene la opción «' + String(option).slice(0, 40) + '»');
    s.selectedIndex = i; s.dispatchEvent(new Event('change', { bubbles: true }));
    // El portal reemplaza el contenido sin recargar la página (~0,5 s): se espera al desplegable NUEVO con la opción elegida, para no leer la tabla anterior.
    const fresh = await waitFor(() => { const n = document.getElementById(String(id)); return n && n !== s && n.selectedIndex >= 0 && norm(n.options[n.selectedIndex].text) === norm(option) ? n : null; }, 10000);
    if (!fresh) return fail('select', 'el portal no actualizó los datos tras elegir «' + String(option).slice(0, 40) + '»');
    return send('step', { ok: true });
  };
  const clickStudent = async (name) => {
    const want = String(name || '').trim().toLowerCase();
    const el = [...document.querySelectorAll('a,button,[role="button"],[role="tab"],option,li')].find((e) => visible(e) && text(e).toLowerCase() === want);
    if (!el) return fail('click_student', 'no se encontró al alumno en el selector');
    el.click();
    return send('step', { ok: true });
  };

  // La app Séneca cambia de pantalla sin cargar página en algunos casos: se vigila la sesión también por tiempo (barato; Rust solo actúa si cambia).
  setInterval(() => { if (location.host === 'seneca.juntadeandalucia.es') session(); }, 3000);
  // Mantiene viva la sesión del servidor (inactividad): pide de nuevo la página actual, sin mostrarla ni cambiar nada. Solo lectura.
  window.__fmPing = () => { if (location.host !== 'seneca.juntadeandalucia.es' || !location.pathname.startsWith('/seneca/nav/')) return; fetch(location.href, { credentials: 'same-origin', cache: 'no-store' }).catch(() => {}); };

  window.__fmRun = async (kind, args) => {
    try {
      let a = {}; try { a = args ? JSON.parse(args) : {}; } catch (_) {}
      if (kind === 'session') return session();
      if (kind === 'recon') return recon();
      if (kind === 'click') return clickText(a.text);
      if (kind === 'select') return selectOption(a.id, a.option);
      if (kind === 'click_student') return clickStudent(a.name);
      if (kind.startsWith('extract:')) {
        const name = kind.slice(8);
        const fn = EXTRACTORS[name];
        if (!fn) return fail(name, 'extractor «' + name + '» sin implementar: falta el reconocimiento del portal');
        return fn(a);
      }
      return fail(kind, 'orden desconocida');
    } catch (e) { return fail(kind, String(e && e.message || e).slice(0, 300)); }
  };
  void sleep;
})();
