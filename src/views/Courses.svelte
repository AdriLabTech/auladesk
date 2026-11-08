<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { listCourses, courseTree, courseCover, downloadActivity, downloadCourse, taskList, type CourseRow, type ActivityRow, type TaskRow } from '../lib/api';
  import { stateInfo, fmtDue } from '../lib/format';
  import TaskDrawer from '../lib/TaskDrawer.svelte';
  import { fmtDate, info, downloadable, hue } from '../lib/format';
  import { takeTarget } from '../lib/nav.svelte';
  import Icon from '../lib/Icon.svelte';

  let courses = $state<CourseRow[]>([]);
  let covers = $state<Record<number, string | null>>({});
  let loaded = $state(false);
  let error = $state('');
  let notice = $state('');
  let open = $state<CourseRow | null>(null);
  let items = $state<ActivityRow[]>([]);
  let filter = $state<'all' | 'assign' | 'files' | 'quiz' | 'forum'>('all');
  let query = $state('');
  let collapsed = $state<Record<number, boolean>>({});
  let busy = $state<number | 'course' | null>(null);
  let progress = $state<{ done: number; total: number; current: string | null } | null>(null);
  let unlisten: UnlistenFn[] = [];

  // Tareas de Moodle por id de actividad: permiten abrir su detalle (descripción, entrega…) desde la propia asignatura
  let courseTasks = $state<Record<number, TaskRow>>({});
  let drawerId = $state<number | null>(null);
  const drawerTask = $derived(drawerId === null ? null : courseTasks[drawerId] ?? null);
  async function loadTasks() { try { courseTasks = Object.fromEntries((await taskList()).map((t) => [t.id, t])); } catch { /* sin datos aún */ } }
  function openTask(id: number) { if (courseTasks[id]) drawerId = id; else error = 'Sincroniza en «Moodle / Conexión» para ver los detalles de esta tarea.'; }
  onMount(async () => {
    loadTasks();
    unlisten.push(await listen<typeof progress>('download-progress', (e) => (progress = e.payload)));
    await reload();
    loaded = true;
    const target = takeTarget('courseId');
    const tc = target === null ? undefined : courses.find((c) => c.id === target);
    if (tc) show(tc);
  });
  onDestroy(() => unlisten.forEach((u) => u()));

  async function reload() {
    try {
      courses = await listCourses();
      for (const c of courses) if (!(c.id in covers)) courseCover(c.id).then((u) => (covers[c.id] = u)).catch(() => (covers[c.id] = null));
      if (open) { open = courses.find((c) => c.id === open!.id) ?? null; if (open) items = await courseTree(open.id); }
    } catch (e) { error = String(e); }
  }
  async function show(c: CourseRow) { loadTasks(); open = c; items = []; filter = 'all'; query = ''; notice = ''; items = await courseTree(c.id).catch((e) => { error = String(e); return []; }); }

  const matches = (r: ActivityRow) => {
    const okFilter = filter === 'all' || (filter === 'assign' && r.modname === 'assign') || (filter === 'files' && (r.modname === 'resource' || r.modname === 'folder')) || (filter === 'quiz' && r.modname === 'quiz') || (filter === 'forum' && r.modname === 'forum');
    return okFilter && (!query.trim() || r.name.toLowerCase().includes(query.trim().toLowerCase()));
  };
  const groups = $derived.by(() => {
    const m = new Map<number, { name: string; all: ActivityRow[]; rows: ActivityRow[] }>();
    for (const r of items) {
      if (!m.has(r.section_position)) m.set(r.section_position, { name: r.section_name, all: [], rows: [] });
      const g = m.get(r.section_position)!; g.all.push(r); if (matches(r)) g.rows.push(r);
    }
    return [...m.entries()].sort((a, b) => a[0] - b[0]);
  });
  const counts = $derived({
    assign: items.filter((i) => i.modname === 'assign').length,
    files: items.filter((i) => i.modname === 'resource' || i.modname === 'folder').length,
    quiz: items.filter((i) => i.modname === 'quiz').length,
    forum: items.filter((i) => i.modname === 'forum').length,
  });

  async function dl(r: ActivityRow) {
    error = ''; notice = ''; busy = r.id;
    try {
      const rep = await downloadActivity(r.id);
      const n = rep.stored.filter((s) => !s.duplicate).length, d = rep.stored.filter((s) => s.duplicate).length;
      notice = `«${r.name}»: ${n} guardado(s)${d ? `, ${d} ya existía(n)` : ''}${rep.errors.length ? ` · Errores: ${rep.errors.join(' | ')}` : ''}`;
      await reload();
    } catch (e) { error = String(e); } finally { busy = null; }
  }
  async function dlAll(onlyMissing: boolean) {
    if (!open) return;
    error = ''; notice = ''; busy = 'course'; progress = null;
    try {
      const rep = await downloadCourse(open.id, onlyMissing);
      notice = `${rep.stored} archivo(s) nuevos, ${rep.duplicates} repetidos, ${rep.errors.length} error(es) en ${rep.activities} actividades.${rep.errors.length ? ' ' + rep.errors.slice(0, 3).join(' | ') : ''}`;
      await reload();
    } catch (e) { error = String(e); } finally { busy = null; progress = null; }
  }
  const jump = (pos: number) => document.getElementById(`tema-${pos}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  const nTasks = (rows: ActivityRow[]) => rows.filter((r) => r.modname === 'assign').length;
</script>

{#snippet cover(c: CourseRow, cls: string)}
  <div class={cls} style={covers[c.id] ? '' : `background: hsl(${hue(c.moodle_id)} 62% 40%)`}>
    {#if covers[c.id]}<img src={covers[c.id]} alt="" />{/if}
  </div>
{/snippet}

{#if open}
  <div class="detail">
    <header class="hero">
      {@render cover(open, 'hero-bg')}
      <div class="hero-in">
        <button class="btn" onclick={() => (open = null)}><Icon name="left" size={15} /> Asignaturas</button>
        <h1>{open.fullname}</h1>
        <div class="stats">
          <span><b>{open.sections}</b> temas</span><span><b>{counts.assign}</b> tareas</span><span><b>{counts.files}</b> archivos de Moodle</span><span><b>{open.files}</b> descargados</span>
        </div>
        <div class="actions">
          <button class="btn primary" disabled={busy !== null} onclick={() => dlAll(true)}>{busy === 'course' ? 'Descargando…' : 'Descargar lo que falta'}</button>
          <button class="btn" disabled={busy !== null} onclick={() => dlAll(false)}>Descargar todo de nuevo</button>
          {#if busy === 'course' && progress}<span class="muted">{progress.done}/{progress.total}{progress.current ? ` · ${progress.current}` : ''}</span>{/if}
        </div>
      </div>
    </header>

    {#if error}<p class="msg err" role="alert">{error}</p>{/if}
    {#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

    <div class="toolbar">
      <div class="chips" role="group" aria-label="Filtrar por tipo">
        <button class="chip" class:on={filter === 'all'} onclick={() => (filter = 'all')}>Todo <i>{items.length}</i></button>
        <button class="chip" class:on={filter === 'assign'} onclick={() => (filter = 'assign')}>Tareas <i>{counts.assign}</i></button>
        <button class="chip" class:on={filter === 'files'} onclick={() => (filter = 'files')}>Archivos <i>{counts.files}</i></button>
        <button class="chip" class:on={filter === 'quiz'} onclick={() => (filter = 'quiz')}>Cuestionarios <i>{counts.quiz}</i></button>
        <button class="chip" class:on={filter === 'forum'} onclick={() => (filter = 'forum')}>Foros <i>{counts.forum}</i></button>
      </div>
      <input class="find" type="search" placeholder="Buscar en este curso…" bind:value={query} aria-label="Buscar en el curso" />
    </div>

    <div class="layout">
      <nav class="toc" aria-label="Temas">
        <h2>Temas</h2>
        {#each groups as [pos, g] (pos)}
          <button class="toc-item" class:dim={g.rows.length === 0} onclick={() => jump(pos)}>
            <span class="toc-n">{pos === 0 ? '·' : pos}</span><span class="toc-t">{g.name}</span><i>{g.rows.length}</i>
          </button>
        {/each}
      </nav>
      <div class="sections">
        {#each groups as [pos, g] (pos)}
          {#if g.rows.length || (filter === 'all' && !query.trim())}
            <section class="sec" id="tema-{pos}">
              <button class="sec-head" onclick={() => (collapsed[pos] = !collapsed[pos])} aria-expanded={!collapsed[pos]}>
                <span class="sec-n">{pos === 0 ? 'General' : `Tema ${pos}`}</span>
                <span class="sec-t">{g.name}</span>
                <span class="sec-meta">{#if nTasks(g.all)}<em>{nTasks(g.all)} tarea{nTasks(g.all) > 1 ? 's' : ''}</em>{/if}{g.all.length} elem.</span>
                <span class="caret" class:shut={collapsed[pos]}><Icon name="down" size={16} /></span>
              </button>
              {#if !collapsed[pos]}
                <ul class="rows">
                  {#each g.rows as r (r.id)}
                    {@const m = info(r.modname)}
                    <li class="row" class:task={r.modname === 'assign'} class:text={r.modname === 'label'}>
                      <span class="ico tone-{m.tone}" title={m.label} aria-hidden="true"><Icon name={m.glyph} size={16} /></span>
                      {#if r.modname === 'assign'}
                        {@const tk = courseTasks[r.id]}
                        <button class="name namebtn" onclick={() => openTask(r.id)} title="Ver descripción, detalles y entregar">
                          <span>{r.name}</span>
                          <small class="muted">{m.label + (tk?.due_at ? ' · vence ' + fmtDue(tk.due_at) : '')}</small>
                        </button>
                        {#if tk}<span class="badge b-{stateInfo[tk.state].tone}">{stateInfo[tk.state].label}</span>{/if}
                        <button class="btn sm primary" onclick={() => openTask(r.id)}>Detalle y entrega</button>
                      {:else}
                        <div class="name"><span>{r.name}</span><small class="muted">{m.label}</small></div>
                      {/if}
                      {#if downloadable(r.modname)}
                        {#if r.files > 0}<span class="have" title="Ya está en tu biblioteca">✓ {r.files} descargado{r.files > 1 ? 's' : ''}</span>{/if}
                        <button class="btn sm" disabled={busy !== null} onclick={() => dl(r)}>{busy === r.id ? 'Descargando…' : r.files > 0 ? 'Actualizar' : 'Descargar'}</button>
                      {/if}
                    </li>
                  {/each}
                  {#if g.rows.length === 0}<li class="row muted">Sin elementos con este filtro.</li>{/if}
                </ul>
              {/if}
            </section>
          {/if}
        {:else}
          <p class="muted">Este curso aún no tiene contenido sincronizado.</p>
        {/each}
      </div>
    </div>
  </div>
{:else}
  <div><h1>Mis asignaturas</h1><p class="muted sub">{courses.length} asignatura{courses.length === 1 ? '' : 's'} sincronizada{courses.length === 1 ? '' : 's'} · pulsa una para ver sus temas y archivos</p>
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if loaded && courses.length === 0}
    <div class="card empty"><span class="ico tone-blue"><Icon name="courses" size={24} /></span><h2>Aún no hay asignaturas</h2><p class="muted">Ve a «Moodle / Conexión», inicia sesión y pulsa «Sincronizar ahora».</p></div>
  {/if}
  <div class="grid">
    {#each courses as c (c.id)}
      <button class="ccard glow" style="--i:{courses.indexOf(c)}" onclick={() => show(c)}>
        {@render cover(c, 'ccover')}
        <div class="cbody">
          <strong>{c.fullname}</strong>
          <div class="pills"><span>{c.sections} temas</span><span>{c.assignments} tareas</span><span>{c.files} archivos</span></div>
          <span class="muted small">Sincronizado: {fmtDate(c.last_synced_at)}</span>
        </div>
      </button>
    {/each}
  </div>
  </div>
{/if}

{#if drawerTask}
  <TaskDrawer task={drawerTask} onClose={() => (drawerId = null)} onChange={async () => { await loadTasks(); await reload(); }} />
{/if}

<style>
  .sub { margin: 4px 0 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 18px; margin-top: 22px; }
  .ccard { padding: 0; overflow: hidden; text-align: left; background: var(--panel); border: 1px solid var(--line); border-radius: 16px; display: flex; flex-direction: column; box-shadow: var(--shadow);
    transition: transform .35s var(--ease), box-shadow .35s; animation: rise .55s var(--ease) backwards; animation-delay: calc(var(--i, 0) * 55ms); }
  .ccard:hover { transform: translateY(-5px); }
  .ccover { height: 124px; position: relative; overflow: hidden; }
  .ccover img, .hero-bg img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform .8s var(--ease); }
  .ccard:hover .ccover img { transform: scale(1.08); }
  .cbody { padding: 14px 16px 16px; display: flex; flex-direction: column; gap: 10px; }
  .cbody strong { font-size: 14.5px; line-height: 1.35; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; min-height: 2.7em; }
  .pills { display: flex; gap: 6px; flex-wrap: wrap; } .pills span { font-size: 11.5px; font-weight: 500; padding: 2px 9px; border-radius: 99px; background: var(--panel-2); color: var(--muted); border: 1px solid var(--line); }
  .small { font-size: 12px; }

  .hero { position: relative; border-radius: 18px; overflow: hidden; border: 1px solid var(--line); margin-bottom: 18px; box-shadow: var(--shadow); }
  .hero-bg { position: absolute; inset: 0; opacity: .4; } .hero-bg img { object-fit: cover; }
  .hero::after { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, var(--bg) 6%, color-mix(in srgb, var(--bg) 40%, transparent) 55%, transparent); }
  .hero-in { position: relative; z-index: 1; padding: 22px 28px 26px; display: flex; flex-direction: column; gap: 14px; align-items: flex-start; }
  .hero h1 { font-size: 28px; margin: 4px 0 0; max-width: 780px; line-height: 1.2; }
  .stats { display: flex; gap: 22px; flex-wrap: wrap; color: var(--muted); } .stats b { color: var(--text); font-size: 18px; margin-right: 3px; }
  .actions { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }

  .toolbar { display: flex; gap: 12px; align-items: center; justify-content: space-between; flex-wrap: wrap; margin-bottom: 16px; }
  .find { min-width: 260px; }

  .layout { display: grid; grid-template-columns: 230px 1fr; gap: 22px; align-items: start; }
  .toc { position: sticky; top: 0; display: flex; flex-direction: column; gap: 2px; max-height: calc(100vh - 170px); overflow: auto; padding-right: 4px; }
  .toc h2 { font-size: 11px; letter-spacing: .1em; text-transform: uppercase; color: var(--muted); margin: 0 0 8px 10px; }
  .toc-item { display: flex; gap: 9px; align-items: baseline; background: none; border: 0; text-align: left; padding: 7px 10px; border-radius: 9px; color: var(--muted); transition: all .18s var(--ease); }
  .toc-item:hover { background: var(--panel-2); color: var(--text); transform: translateX(3px); } .toc-item.dim { opacity: .4; }
  .toc-n { font-variant-numeric: tabular-nums; min-width: 16px; color: var(--accent); font-weight: 700; } .toc-t { flex: 1; font-size: 12.5px; line-height: 1.35; } .toc-item i { font-style: normal; font-size: 11px; }
  @media (max-width: 960px) { .layout { grid-template-columns: 1fr; } .toc { display: none; } }

  .sections { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .sec { background: var(--panel); border: 1px solid var(--line); border-radius: 16px; overflow: hidden; scroll-margin-top: 12px; box-shadow: var(--shadow); animation: rise .5s var(--ease) backwards; }
  .sec-head { width: 100%; display: flex; align-items: center; gap: 14px; padding: 14px 18px; background: none; border: 0; text-align: left; transition: background .18s; }
  .sec-head:hover { background: var(--panel-2); }
  .sec-n { font-size: 11px; font-weight: 700; letter-spacing: .07em; text-transform: uppercase; color: var(--accent); white-space: nowrap; padding: 3px 9px; border-radius: 99px; background: var(--accent-soft); }
  .sec-t { flex: 1; font-weight: 600; min-width: 0; }
  .sec-meta { font-size: 12px; color: var(--muted); white-space: nowrap; } .sec-meta em { font-style: normal; color: var(--warn); margin-right: 10px; font-weight: 600; }
  .caret { transition: transform .3s var(--ease-spring); color: var(--muted); display: grid; } .caret.shut { transform: rotate(-90deg); }
  .rows { list-style: none; margin: 0; padding: 0; border-top: 1px solid var(--line); animation: fade .3s backwards; }
  .row { display: flex; align-items: center; gap: 14px; padding: 10px 18px; border-top: 1px solid var(--line); transition: background .15s; } .row:first-child { border-top: 0; } .row:hover { background: var(--panel-2); }
  .row.task { background: color-mix(in srgb, var(--warn) 6%, transparent); } .row.task:hover { background: color-mix(in srgb, var(--warn) 11%, transparent); }
  .row.text .name span { color: var(--muted); font-style: italic; }
  .namebtn { background: none; border: 0; text-align: left; padding: 2px 0; color: inherit; cursor: pointer; } .namebtn:hover span { color: var(--accent); text-decoration: underline; text-underline-offset: 3px; }
  .badge { --t: var(--t-badge, var(--c-muted)); } .b-green { --t-badge: var(--c-green); } .b-red { --t-badge: var(--c-red); } .b-teal { --t-badge: var(--c-teal); } .b-violet { --t-badge: var(--c-violet); } .b-blue { --t-badge: var(--c-blue); } .b-muted { --t-badge: var(--c-muted); }
  .name { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; } .name span { overflow-wrap: anywhere; font-weight: 500; } .name small { font-size: 11.5px; }
  .have { font-size: 12px; color: var(--ok); white-space: nowrap; font-weight: 600; } .btn.sm { white-space: nowrap; }
</style>
