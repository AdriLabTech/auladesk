// Puente + selectores v1. Solo LEE el DOM de la página que el usuario ya está viendo. Sin regex sobre HTML.
(() => {
  if (window.__mdBridge) return;
  const VERSION = 'v1';
  window.__mdBridge = VERSION;

  const rawSend = (kind, payload, url) => {
    const inv = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
    if (!inv) return Promise.resolve();
    return inv('ingest_page', { kind, url: url || location.href, payload }).catch((e) => {
      try { document.title = '[mdbridge error-' + kind + ': ' + String(e).slice(0, 300) + '] ' + document.title; } catch (_) {}
    });
  };
  const send = (kind, payload) => rawSend(kind, payload, location.href);
  const LIVE = () => ({ doc: document, loc: location, wait: waitFor });
  const cfg = () => (window.M && window.M.cfg) || null;
  const text = (el) => (el ? el.textContent.replace(/\s+/g, ' ').trim() : '');
  const idFromUrl = (u, re) => { const m = re.exec(u || ''); return m ? Number(m[1]) : null; };
  const waitFor = async (pred, ms = 12000) => {
    const t0 = Date.now();
    while (Date.now() - t0 < ms) { const v = pred(); if (v) return v; await new Promise((r) => setTimeout(r, 250)); }
    return null;
  };
  const fail = (kind, why) => send('error', { kind, selector_version: VERSION, message: why });

  // Nombre de la cuenta: primero el menú de usuario de la página; si el tema no lo trae, el perfil propio (mismo origen, con la sesión del navegador).
  const accountName = async (c) => {
    const uid = Number(c.userId) || 0;
    if (!uid) return null;
    for (const s of ['.usermenu .usertext', '#user-menu-toggle .usertext', '[data-region="usermenu"] .usertext', '.userbutton .usertext']) {
      const t = text(document.querySelector(s));
      if (t) return t;
    }
    try {
      const k = 'mdname:' + uid, cached = sessionStorage.getItem(k);
      if (cached) return cached;
      const r = await fetch(String(c.wwwroot || '') + '/user/profile.php?id=' + uid, { credentials: 'same-origin' });
      if (!r.ok) return null;
      const d = new DOMParser().parseFromString(await r.text(), 'text/html');
      const t = text(d.querySelector('.page-header-headings h1, #page-header h1, h1')) || text({ textContent: (d.title || '').split(':')[0] });
      if (t) sessionStorage.setItem(k, t);
      return t || null;
    } catch (_) { return null; }
  };
  const reportSession = async () => {
    const c = cfg();
    if (!c) return;
    const fullName = await accountName(c);
    send('session', { userId: Number(c.userId) || 0, timezone: c.usertimezone || null, fullName });
  };
  document.addEventListener('DOMContentLoaded', reportSession);

  // Portada: imagen de la tarjeta. Si es una URL del propio Moodle se lee con la sesión del navegador (same-origin).
  async function coverOf(card) {
    try {
      const el = card.querySelector('.card-img-top');
      const m = el && /url\(["']?(.*?)["']?\)/.exec(el.style.backgroundImage || '');
      if (!m) return null;
      let u = m[1];
      if (u.startsWith('data:')) return u.length < 300000 ? u : null;
      if (new URL(u, location.href).origin !== location.origin) return null;
      const r = await fetch(u, { credentials: 'same-origin' });
      if (!r.ok) return null;
      const b = await r.blob();
      if (!b.type.startsWith('image/') || b.size > 8000000) return null;
      if (b.type === 'image/svg+xml') return b.size < 250000 ? await new Promise((res) => { const f = new FileReader(); f.onload = () => res(f.result); f.onerror = () => res(null); f.readAsDataURL(b); }) : null;
      // Imagen real: se reduce a 640 px de ancho (JPEG) para que la portada pese poco.
      const bmp = await createImageBitmap(b);
      const w = Math.min(640, bmp.width), h = Math.round(bmp.height * (w / bmp.width));
      const cv = document.createElement('canvas'); cv.width = w; cv.height = h;
      const cx = cv.getContext('2d'); cx.fillStyle = '#fff'; cx.fillRect(0, 0, w, h); cx.drawImage(bmp, 0, 0, w, h);
      const out = cv.toDataURL('image/jpeg', 0.82);
      return out.length < 400000 ? out : null;
    } catch (_) { return null; }
  }

  // --- Lista de cursos (/my/courses.php) ---
  async function courses() {
    if (!location.pathname.endsWith('/my/courses.php')) return fail('courses', 'no estoy en /my/courses.php');
    const view = await waitFor(() => document.querySelector('[data-region="courses-view"]'));
    if (!view) return fail('courses', 'estructura: falta [data-region=courses-view]');
    const deck = await waitFor(() => view.querySelector('[data-region="card-deck"]') || view.querySelector('[data-region="empty-message"]'));
    if (!deck) return fail('courses', 'estructura: ni tarjetas ni mensaje de vacío');
    if (view.dataset.display !== 'card') return fail('courses', 'vista no es de tarjetas: ' + view.dataset.display);
    if (view.dataset.grouping !== 'all') return fail('courses', 'filtro no es "Todos": ' + view.dataset.grouping);
    const cards = [...view.querySelectorAll('[data-region="course-content"][data-course-id]')];
    const list = [];
    for (const card of cards) {
      const a = card.querySelector('a.coursename');
      const name = (card.querySelector('.multiline') || {}).title || text(a);
      list.push({ moodle_id: Number(card.dataset.courseId), name, url: a ? a.href : null, cover: await coverOf(card) });
    }
    const pagingNext = view.querySelector('[data-region="paging-bar"] [data-control="next"]:not(.disabled)');
    return send('courses', {
      selector_version: VERSION, total_count: Number(view.dataset.totalcoursecount) || null,
      more_pages: !!pagingNext, courses: list,
    });
  }

  // --- Contenido de un curso (/course/view.php?id=N, formato temas/semanas) ---
  async function contents(ctx) {
    const { doc, loc, wait } = ctx || LIVE();
    const send = (k, p) => rawSend(k, p, loc.href);
    if (!loc.pathname.endsWith('/course/view.php')) return fail('course_contents', 'no estoy en course/view.php');
    const courseId = idFromUrl(loc.search, /[?&]id=(\d+)/);
    const first = await wait(() => doc.querySelector('li.course-section[data-number]'));
    if (!first) return fail('course_contents', 'estructura: sin li.course-section (formato de curso no soportado)');
    const sections = [...doc.querySelectorAll('li.course-section[data-number]')].map((s) => ({
      moodle_id: Number(s.dataset.id) || null,
      position: Number(s.dataset.number),
      name: s.dataset.sectionname || text(s.querySelector('.sectionname')) || 'Sin nombre',
      activities: [...s.querySelectorAll('li.activity[data-id]')].map((li) => {
        const modname = ([...li.classList].find((c) => c.startsWith('modtype_')) || 'modtype_unknown').slice(8);
        const link = li.querySelector('.activityname a[href]');
        const inst = li.querySelector('.instancename');
        let name = '';
        if (inst) { const c = inst.cloneNode(true); c.querySelectorAll('.accesshide').forEach((n) => n.remove()); name = text(c); }
        if (!name) name = text(li.querySelector('.activity-description, .contentwithoutlink')).slice(0, 80);
        return {
          cmid: Number(li.dataset.id), modname, name: name || '(sin nombre)',
          url: link ? link.href : null,
          description: text(li.querySelector('.activity-description')) || null,
          due_at: null, files: [],
        };
      }),
    }));
    return send('course_contents', { selector_version: VERSION, course_id: courseId, sections });
  }

  // --- Página de una tarea (/mod/assign/view.php?id=N) ---
  async function assignment(ctx) {
    const { doc, loc, wait } = ctx || LIVE();
    const send = (k, p) => rawSend(k, p, loc.href);
    if (!loc.pathname.endsWith('/mod/assign/view.php')) return fail('assignment', 'no estoy en mod/assign/view.php');
    const cmid = idFromUrl(loc.search, /[?&]id=(\d+)/);
    await wait(() => doc.querySelector('#region-main'));
    const title = doc.title;
    const table = doc.querySelector('.submissionstatustable');
    const err = doc.querySelector('.errorbox, .alert-danger, [data-region="error"]');
    if (/^error/i.test(title) || (err && !table)) return send('assignment', { selector_version: VERSION, cmid, page_title: title, error: text(err) || title });
    const rowsOf = (root) => [...(root ? root.querySelectorAll('table tr') : [])].map((tr) => {
      const th = tr.querySelector('th'), td = tr.querySelector('td');
      return th && td ? { label: text(th), value: text(td) } : null;
    }).filter(Boolean);
    const datesEl = doc.querySelector('[data-region="activity-dates"]');
    let dates = datesEl ? [...datesEl.querySelectorAll(':scope > div')].map(text).filter(Boolean) : [];
    if (datesEl && dates.length <= 1) dates = text(datesEl).split(/(?=(?:Apertura|Cierre|Fecha de entrega|Abierto|Vencimiento):)/).map((x) => x.trim()).filter(Boolean);
    const files = [...(table || document).querySelectorAll('.fileuploadsubmission a[href*="/pluginfile.php/"]')].map((a) => {
      const box = a.closest('.fileuploadsubmission');
      const t = box && box.nextElementSibling && box.nextElementSibling.classList.contains('fileuploadsubmissiontime') ? text(box.nextElementSibling) : null;
      return { name: text(a), url: a.href.split('#')[0], time_text: t };
    });
    const btns = [...doc.querySelectorAll('#region-main .btn, #region-main button, #region-main a.btn')].map(text);
    // Descripción de la tarea como TEXTO plano (listas con viñetas, saltos de línea) + sus enlaces. Nunca se envía HTML.
    const introEl = doc.querySelector('#intro') || doc.querySelector('.activity-description');
    const BLOCK = new Set(['P', 'DIV', 'UL', 'OL', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'TABLE', 'TR', 'PRE', 'BLOCKQUOTE', 'SECTION', 'ARTICLE']);
    const toText = (root) => {
      const out = [];
      const walk = (n) => {
        if (n.nodeType === 3) { out.push(n.textContent.replace(/\s+/g, ' ')); return; }
        if (n.nodeType !== 1) return;
        const tag = n.tagName.toUpperCase();
        if (['SCRIPT', 'STYLE', 'NOSCRIPT', 'SVG', 'IMG'].includes(tag)) return;
        if (tag === 'BR') { out.push('\n'); return; }
        if (tag === 'LI') out.push('\n• '); else if (BLOCK.has(tag)) out.push('\n');
        for (const c of n.childNodes) walk(c);
        if (tag === 'TD' || tag === 'TH') out.push(' | ');
        if (BLOCK.has(tag)) out.push('\n');
      };
      walk(root);
      return out.join('').replace(/[ \t]+\n/g, '\n').replace(/ \| \n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
    };
    const intro_text = introEl ? toText(introEl).slice(0, 20000) : null;
    const seenUrl = new Set();
    const intro_links = [...new Set([...(introEl ? introEl.querySelectorAll('a[href]') : []), ...doc.querySelectorAll('.introattachments a[href]')])]
      .map((a) => ({ name: text(a) || a.href, url: a.href.split('#')[0], file: /\/pluginfile\.php\//.test(a.href) }))
      .filter((l) => /^https:/.test(l.url) && !seenUrl.has(l.url) && seenUrl.add(l.url)).slice(0, 60);
    return send('assignment', {
      intro_text, intro_links,
      selector_version: VERSION, cmid, page_title: title, dates,
      status_rows: rowsOf(table), grade_rows: rowsOf(doc.querySelector('.feedback .feedbacktable')),
      files, can_add: btns.some((b) => /agregar entrega/i.test(b)), can_edit: btns.some((b) => /editar entrega/i.test(b)),
    });
  }

  // --- Fechas de una actividad que no es tarea (cuestionarios…) ---
  async function activityDates(ctx) {
    const { doc, loc, wait } = ctx || LIVE();
    const send = (k, p) => rawSend(k, p, loc.href);
    const m = /\/mod\/([a-z]+)\/view\.php$/.exec(loc.pathname);
    if (!m) return fail('activity_dates', 'no estoy en una página mod/*/view.php');
    const cmid = idFromUrl(loc.search, /[?&]id=(\d+)/);
    await wait(() => doc.querySelector('#region-main'));
    if (/^error/i.test(doc.title)) return send('activity_dates', { selector_version: VERSION, cmid, modname: m[1], error: doc.title });
    const el = doc.querySelector('[data-region="activity-dates"]');
    let dates = el ? [...el.querySelectorAll(':scope > div')].map(text).filter(Boolean) : [];
    if (el && dates.length <= 1) dates = text(el).split(/(?=(?:Apertura|Cierre|Abrió|Cerró|Se abrirá|Se cerrará|Abierto|Cerrado|Fecha de entrega|Vencimiento):)/).map((x) => x.trim()).filter(Boolean);
    return send('activity_dates', { selector_version: VERSION, cmid, modname: m[1], dates });
  }

  // --- Calendario de Moodle: vista «Próximos eventos» ---
  async function calendarUpcoming(ctx) {
    const { doc, loc, wait } = ctx || LIVE();
    const send = (k, p) => rawSend(k, p, loc.href);
    if (!loc.pathname.endsWith('/calendar/view.php')) return fail('calendar', 'no estoy en calendar/view.php');
    await wait(() => doc.querySelector('.eventlist, [data-region="calendar"], #region-main'));
    const events = [...doc.querySelectorAll('.eventlist [data-event-id]')].map((e) => {
      const date = e.querySelector('[data-timestamp]');
      const a = e.querySelector('a[href*="/mod/"]');
      return { id: Number(e.dataset.eventId), title: e.dataset.eventTitle || text(e.querySelector('.name')), component: e.dataset.eventComponent || '',
        eventtype: e.dataset.eventEventtype || '', course_id: Number(e.dataset.courseId) || null, timestamp: date ? Number(date.dataset.timestamp) : null, url: a ? a.href : null };
    });
    return send('calendar', { selector_version: VERSION, events });
  }

  // --- Archivos de una actividad (recurso, carpeta o tarea) ---
  async function files() {
    // Algunos recursos redirigen directamente al archivo: la propia página (visor PDF de WebKit) ES el archivo.
    if (location.pathname.includes('/pluginfile.php/')) {
      const u = location.href.split('#')[0];
      return send('files', { selector_version: VERSION, files: [{ url: u, name: decodeURIComponent(u.split('?')[0].split('/').pop() || '') }] });
    }
    await waitFor(() => document.querySelector('#region-main'), 8000);
    const main = document.querySelector('#region-main') || document.body;
    const found = new Map();
    main.querySelectorAll('a[href*="/pluginfile.php/"]').forEach((a) => found.set(a.href, text(a) || null));
    main.querySelectorAll('object[data*="/pluginfile.php/"], iframe[src*="/pluginfile.php/"], embed[src*="/pluginfile.php/"]').forEach((o) => found.set(o.data || o.src, null));
    return send('files', { selector_version: VERSION, files: [...found].map(([url, name]) => ({ url, name })) });
  }

  // --- Entrega desde la app (solo se invoca tras la confirmación explícita del usuario, vía submission_submit) ---
  // Cada etapa se detiene ante cualquier duda y avisa con submit_stage {ok:false}; nunca adivina ni reintenta a ciegas.
  window.__mdSubmit = {
    name: '', parts: [], replace: false,
    begin(name, replace) { this.name = String(name || ''); this.parts = []; this.replace = replace === true; },
    chunk(b64) { const bin = atob(b64), u = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i); this.parts.push(u); },
    file() { return new File(this.parts, this.name); },
  };
  // OJO: los formularios de Moodle llevan un <input name="action"> que pisa form.action: se lee siempre con getAttribute.
  const inForm = (name, value) => [...document.querySelectorAll('form')].find((f) => /\/mod\/assign\/view\.php/.test(f.getAttribute('action') || '') && f.querySelector(`input[name="${name}"][value="${value}"]`));
  async function submitStep(kind) {
    const bad = (message) => send('submit_stage', { ok: false, stage: kind, message });
    const done = (extra) => send('submit_stage', Object.assign({ ok: true, stage: kind }, extra || {}));
    if (!/\/mod\/assign\/view\.php$/.test(location.pathname)) return bad('No estoy en una página de tarea de Moodle.');
    if (kind === 'submit_stage') {
      if (!/[?&]action=editsubmission/.test(location.search)) return bad('No estoy en la página de edición de la entrega.');
      const fm = await waitFor(() => document.querySelector('.filemanager'), 15000);
      if (!fm) return bad('No se encontró el selector de archivos de Moodle (¿entrega cerrada o de otro tipo?).');
      const existing = () => [...fm.querySelectorAll('.fp-file')];
      if (existing().length || fm.querySelector('.fp-filename')) {
        if (!window.__mdSubmit.replace) return bad('La entrega ya contiene archivos. AulaDesk no los sustituye sin que lo pidas: bórralos en Moodle o usa «Reemplazar entrega».');
        // Modificar entrega: se eliminan los archivos anteriores en el formulario. Nada se guarda hasta «Guardar cambios», así que parar aquí no cambia la entrega.
        for (let guard = 0; existing().length && guard < 10; guard++) {
          const before = existing().length, el = existing()[0];
          (el.querySelector('a') || el).click();
          const del = await waitFor(() => document.querySelector('.fp-file-delete'), 5000);
          if (!del) return bad('No encontré cómo eliminar el archivo anterior de la entrega. No se ha cambiado nada.');
          del.click();
          const ok = await waitFor(() => document.querySelector('.fp-dlg-butconfirm'), 5000);
          if (!ok) return bad('Moodle no pidió confirmar la eliminación del archivo anterior. No se ha cambiado nada.');
          ok.click();
          if (!(await waitFor(() => existing().length < before, 8000))) return bad('No se pudo eliminar el archivo anterior. No se ha cambiado nada.');
        }
        if (existing().length) return bad('No se pudieron eliminar todos los archivos anteriores. No se ha cambiado nada.');
      }
      const f = window.__mdSubmit.file();
      if (!f || !f.size) return bad('El archivo no llegó a la página.');
      const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
      // Pistas para afinar si Moodle usa otras clases: solo nombres de clase de la página, nunca contenido.
      const diag = () => {
        const cls = new Set();
        document.querySelectorAll('[class*="file"],[class*="fp-"],[class*="dialog"],[class*="modal"]').forEach((n) => n.classList.forEach((c) => { if (/file|fp-|dialog|modal/i.test(c)) cls.add(c); }));
        const a0 = fm.querySelector('.fp-btn-add');
        return ` [diag: add=${a0 ? a0.outerHTML.slice(0, 140).replace(/\s+/g, ' ') : 'no'}; loading=${!!fm.classList.contains('fm-loading')}; clases=${[...cls].slice(0, 30).join(' ')}]`;
      };
      const PICKER = '.file-picker, .fp-generallayout, .filepicker, .fp-repo-area, .fp-content-area';
      const findPicker = () => { const el = document.querySelector(PICKER); return el ? (el.closest('.file-picker, .filepicker, .moodle-dialogue-base, .modal') || el) : null; };
      // El gestor de archivos de Moodle (YUI) engancha sus manejadores DESPUÉS de cargar la página: se espera y se reintenta el clic.
      await waitFor(() => document.readyState === 'complete', 15000);
      await sleep(1500);
      let dlg = null;
      for (let i = 0; i < 4 && !dlg; i++) {
        dlg = findPicker();
        if (dlg) break;
        const add = fm.querySelector('.fp-btn-add a, .fp-btn-add button') || fm.querySelector('.fp-btn-add');
        if (!add) return bad('No se encontró el botón «Agregar…».' + diag());
        add.click();
        dlg = await waitFor(findPicker, 4000);
        if (!dlg) await sleep(1500);
      }
      if (!dlg) return bad('No se abrió el selector de archivos de Moodle.' + diag());
      const scope = () => dlg.isConnected ? dlg : document;
      const repo = [...scope().querySelectorAll('.fp-repo a, .fp-repo')].find((r) => /subir un archivo|upload a file/i.test(r.textContent));
      if (repo) repo.click();
      const input = await waitFor(() => scope().querySelector('.fp-upload-form input[type="file"]') || document.querySelector('.fp-upload-form input[type="file"]') || scope().querySelector('input[type="file"]'), 8000);
      if (!input) return bad('No se encontró el campo de subida de Moodle.' + diag());
      const dt = new DataTransfer(); dt.items.add(f); input.files = dt.files; input.dispatchEvent(new Event('change', { bubbles: true }));
      const btn = scope().querySelector('.fp-upload-btn') || document.querySelector('.fp-upload-btn') || (input.closest('form') && input.closest('form').querySelector('button, input[type="submit"]'));
      if (!btn) return bad('No se encontró «Subir este archivo».' + diag());
      btn.click();
      const ok = await waitFor(() => [...fm.querySelectorAll('.fp-filename')].some((n) => text(n) === f.name), 90000);
      if (!ok) return bad(text(document.querySelector('.fp-msg-error, .moodle-exception-message, .alert-danger')) || 'Moodle no confirmó la subida del archivo.');
      return done();
    }
    if (kind === 'submit_save') {
      const btn = document.querySelector('#id_submitbutton'), form = btn && btn.closest('form');
      if (!btn || !form || !/\/mod\/assign\/view\.php/.test(form.getAttribute('action') || '')) return bad('No se encontró «Guardar cambios».');
      if (!document.querySelector('.filemanager .fp-filename')) return bad('El archivo no está en el formulario de entrega.');
      btn.click(); return;
    }
    if (kind === 'submit_final') {   // la tarea puede quedar en borrador: botón «Enviar tarea»
      const f = inForm('action', 'submit');
      if (!f) return done({ needs_final: false });
      const b = f.querySelector('button[type="submit"], input[type="submit"]');
      if (!b) return bad('Se vio el formulario de envío pero no su botón.');
      b.click(); return;
    }
    if (kind === 'submit_confirm') {  // pantalla «¿Seguro que quiere enviar su trabajo?»
      const f = inForm('action', 'confirmsubmit');
      if (!f) return bad('No se encontró la pantalla de confirmación del envío.');
      const b = f.querySelector('#id_submitbutton') || f.querySelector('button[type="submit"], input[type="submit"]');
      if (!b) return bad('No se encontró el botón «Continuar» del envío.');
      b.click(); return;
    }
    return bad('etapa desconocida');
  }


  // --- Modo sin renderizar: pide solo el HTML (mismo origen, con la sesión del navegador) y extrae del documento analizado. ---
  const EXTRACT = { contents, assignment, activity_dates: activityDates, calendar: calendarUpcoming };
  async function fetchRun(kind, url) {
    const ex = EXTRACT[kind];
    if (!ex) return fail(kind, 'sin modo fetch para ' + kind);
    let r;
    try { r = await fetch(url, { credentials: 'same-origin', redirect: 'follow' }); } catch (e) { return fail(kind, 'fetch: ' + String(e).slice(0, 200)); }
    if (!r.ok) return fail(kind, 'fetch: HTTP ' + r.status);
    const final = new URL(r.url);
    if (/\/login\//.test(final.pathname)) return fail(kind, 'fetch: la sesión no es válida (redirigido a login)');
    const doc = new DOMParser().parseFromString(await r.text(), 'text/html');
    const base = doc.createElement('base'); base.href = r.url; doc.head.prepend(base); // los enlaces relativos se resuelven contra la página pedida
    return ex({ doc, loc: final, wait: async (p) => p() });
  }
  window.__mdFetch = (kind, url) => { try { return fetchRun(kind, url); } catch (e) { return fail(kind, String(e)); } };

  window.__mdRun = (kind) => {
    try {
      if (kind.startsWith('submit_')) return submitStep(kind);
      if (kind === 'session') return reportSession();
      if (kind === 'courses') return courses();
      if (kind === 'contents') return contents();
      if (kind === 'files') return files();
      if (kind === 'assignment') return assignment();
      if (kind === 'activity_dates') return activityDates();
      if (kind === 'calendar') return calendarUpcoming();
      if (kind === 'logout') { const c = cfg(); if (c && c.sesskey) location.href = c.wwwroot + '/login/logout.php?sesskey=' + encodeURIComponent(c.sesskey); return; }
      if (kind === 'dom') {
        const clone = (document.querySelector('#page') || document.body).cloneNode(true);
        clone.querySelectorAll('script,style,noscript,svg,input[type=hidden],input[type=password]').forEach((n) => n.remove());
        clone.querySelectorAll('input,textarea').forEach((n) => n.removeAttribute('value'));
        return send('dom', { title: document.title, bodyClass: document.body.className, html: clone.outerHTML });
      }
      return fail(kind, 'probe desconocido');
    } catch (e) { return fail(kind, String(e)); }
  };
})();
