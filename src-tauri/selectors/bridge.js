// Puente + selectores v1. Solo LEE el DOM de la página que el usuario ya está viendo. Sin regex sobre HTML.
(() => {
  if (window.__mdBridge) return;
  const VERSION = 'v1';
  window.__mdBridge = VERSION;

  const send = (kind, payload) => {
    const inv = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
    if (!inv) return Promise.resolve();
    return inv('ingest_page', { kind, url: location.href, payload }).catch((e) => {
      try { document.title = '[mdbridge error-' + kind + ': ' + String(e).slice(0, 300) + '] ' + document.title; } catch (_) {}
    });
  };
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
  async function contents() {
    if (!location.pathname.endsWith('/course/view.php')) return fail('course_contents', 'no estoy en course/view.php');
    const courseId = idFromUrl(location.search, /[?&]id=(\d+)/);
    const first = await waitFor(() => document.querySelector('li.course-section[data-number]'));
    if (!first) return fail('course_contents', 'estructura: sin li.course-section (formato de curso no soportado)');
    const sections = [...document.querySelectorAll('li.course-section[data-number]')].map((s) => ({
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
  async function assignment() {
    if (!location.pathname.endsWith('/mod/assign/view.php')) return fail('assignment', 'no estoy en mod/assign/view.php');
    const cmid = idFromUrl(location.search, /[?&]id=(\d+)/);
    await waitFor(() => document.querySelector('#region-main'));
    const title = document.title;
    const table = document.querySelector('.submissionstatustable');
    const err = document.querySelector('.errorbox, .alert-danger, [data-region="error"]');
    if (/^error/i.test(title) || (err && !table)) return send('assignment', { selector_version: VERSION, cmid, page_title: title, error: text(err) || title });
    const rowsOf = (root) => [...(root ? root.querySelectorAll('table tr') : [])].map((tr) => {
      const th = tr.querySelector('th'), td = tr.querySelector('td');
      return th && td ? { label: text(th), value: text(td) } : null;
    }).filter(Boolean);
    const datesEl = document.querySelector('[data-region="activity-dates"]');
    let dates = datesEl ? [...datesEl.querySelectorAll(':scope > div')].map(text).filter(Boolean) : [];
    if (datesEl && dates.length <= 1) dates = text(datesEl).split(/(?=(?:Apertura|Cierre|Fecha de entrega|Abierto|Vencimiento):)/).map((x) => x.trim()).filter(Boolean);
    const files = [...(table || document).querySelectorAll('.fileuploadsubmission a[href*="/pluginfile.php/"]')].map((a) => {
      const box = a.closest('.fileuploadsubmission');
      const t = box && box.nextElementSibling && box.nextElementSibling.classList.contains('fileuploadsubmissiontime') ? text(box.nextElementSibling) : null;
      return { name: text(a), url: a.href.split('#')[0], time_text: t };
    });
    const btns = [...document.querySelectorAll('#region-main .btn, #region-main button, #region-main a.btn')].map(text);
    return send('assignment', {
      selector_version: VERSION, cmid, page_title: title, dates,
      status_rows: rowsOf(table), grade_rows: rowsOf(document.querySelector('.feedback .feedbacktable')),
      files, can_add: btns.some((b) => /agregar entrega/i.test(b)), can_edit: btns.some((b) => /editar entrega/i.test(b)),
    });
  }

  // --- Fechas de una actividad que no es tarea (cuestionarios…) ---
  async function activityDates() {
    const m = /\/mod\/([a-z]+)\/view\.php$/.exec(location.pathname);
    if (!m) return fail('activity_dates', 'no estoy en una página mod/*/view.php');
    const cmid = idFromUrl(location.search, /[?&]id=(\d+)/);
    await waitFor(() => document.querySelector('#region-main'));
    if (/^error/i.test(document.title)) return send('activity_dates', { selector_version: VERSION, cmid, modname: m[1], error: document.title });
    const el = document.querySelector('[data-region="activity-dates"]');
    let dates = el ? [...el.querySelectorAll(':scope > div')].map(text).filter(Boolean) : [];
    if (el && dates.length <= 1) dates = text(el).split(/(?=(?:Apertura|Cierre|Abrió|Cerró|Se abrirá|Se cerrará|Abierto|Cerrado|Fecha de entrega|Vencimiento):)/).map((x) => x.trim()).filter(Boolean);
    return send('activity_dates', { selector_version: VERSION, cmid, modname: m[1], dates });
  }

  // --- Calendario de Moodle: vista «Próximos eventos» ---
  async function calendarUpcoming() {
    if (!location.pathname.endsWith('/calendar/view.php')) return fail('calendar', 'no estoy en calendar/view.php');
    await waitFor(() => document.querySelector('.eventlist, [data-region="calendar"], #region-main'));
    const events = [...document.querySelectorAll('.eventlist [data-event-id]')].map((e) => {
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

  window.__mdRun = (kind) => {
    try {
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
