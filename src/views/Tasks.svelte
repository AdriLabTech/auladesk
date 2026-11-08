<script lang="ts">
  import { onMount } from 'svelte';
  import { taskList, taskSetFlags, noteSet, downloadActivity, moodleOpen, moodleNavigate, getSetting, setSetting, listFiles, type TaskRow, type TaskState, type FileRow } from '../lib/api';
  import { fmtDue, fmtRemaining, fmtDate, stateInfo, hue } from '../lib/format';
  import TaskDetail from './TaskDetail.svelte';
  import TaskDrawer from '../lib/TaskDrawer.svelte';
  import Icon from '../lib/Icon.svelte';
  import { takeTarget } from '../lib/nav.svelte';
  import { openViewer, kindOf } from '../lib/viewerstate.svelte';
  import { fmtSize } from '../lib/format';

  type View = 'list' | 'board';
  type By = 'state' | 'course' | 'priority';
  type Col = { id: string; title: string; color: string; hint?: string; items: TaskRow[] };

  let tasks = $state<TaskRow[]>([]);
  let loaded = $state(false);
  let error = $state(''); let notice = $state('');
  let view = $state<View>('list'); let by = $state<By>('state'); let showStale = $state(false);
  let group = $state<'todo' | 'all' | 'done'>('todo');
  let courseId = $state(''); let q = $state(''); let sort = $state<'due' | 'priority' | 'course' | 'state'>('due');
  let openId = $state<number | null>(null);

  const todoStates: TaskState[] = ['pendiente', 'iniciada', 'preparada', 'fuera_de_plazo', 'sin_fecha', 'sin_sincronizar'];
  const doneStates: TaskState[] = ['entregada', 'evaluada'];
  const order: Record<string, number> = { fuera_de_plazo: 0, pendiente: 1, iniciada: 2, preparada: 3, sin_fecha: 4, sin_sincronizar: 5, entregada: 6, evaluada: 7 };
  const stale = (t: TaskRow) => t.state === 'fuera_de_plazo' && t.seconds_to_due !== null && t.seconds_to_due <= -30 * 86400;

  async function load() { try { tasks = await taskList(); } catch (e) { error = String(e); } loaded = true; }
  onMount(async () => {
    try {
      const v = await getSetting('tasks_view'), b = await getSetting('tasks_board_by');
      if (v === 'board' || v === 'list') view = v; if (b === 'state' || b === 'course' || b === 'priority') by = b;
    } catch { /* ajustes aún no disponibles */ }
    await load();
    const target = takeTarget('taskId'); const t = target === null ? undefined : tasks.find((x) => x.id === target);
    if (t) { group = 'all'; if (stale(t)) showStale = true; toggle(t); setTimeout(() => document.getElementById(`task-${t.id}`)?.scrollIntoView({ behavior: 'smooth', block: 'center' }), 120); }
  });
  const setView = (v: View) => { view = v; setSetting('tasks_view', v).catch(console.error); };
  const setBy = (b: By) => { by = b; setSetting('tasks_board_by', b).catch(console.error); };

  const courses = $derived([...new Map(tasks.map((t) => [t.course_id, t.course_name])).entries()]);
  const counts = $derived({ todo: tasks.filter((t) => todoStates.includes(t.state)).length, done: tasks.filter((t) => doneStates.includes(t.state)).length, late: tasks.filter((t) => t.state === 'fuera_de_plazo' && !stale(t)).length });
  const filtered = $derived(tasks.filter((t) => (!courseId || String(t.course_id) === courseId) && (!q.trim() || (t.name + ' ' + t.course_name).toLowerCase().includes(q.trim().toLowerCase()))));

  const dueAsc = (a: TaskRow, b: TaskRow) => (a.due_at ?? Infinity) - (b.due_at ?? Infinity);
  const dueDesc = (a: TaskRow, b: TaskRow) => (b.due_at ?? 0) - (a.due_at ?? 0);

  const shown = $derived.by(() => {
    const f = filtered.filter((t) => group === 'all' || (group === 'todo' ? todoStates.includes(t.state) : doneStates.includes(t.state)));
    const byFn: Record<string, (a: TaskRow, b: TaskRow) => number> = {
      due: dueAsc,
      priority: (a, b) => b.priority - a.priority || dueAsc(a, b),
      course: (a, b) => a.course_name.localeCompare(b.course_name) || dueAsc(a, b),
      state: (a, b) => order[a.state] - order[b.state] || dueAsc(a, b),
    };
    // Con «por hacer»: vencidas recientes (≤30 d) primero, después lo próximo por fecha, sin fecha, y al final las vencidas antiguas (restos de cursos pasados).
    const rank = (t: TaskRow) => {
      if (t.state === 'fuera_de_plazo') return t.seconds_to_due !== null && t.seconds_to_due > -30 * 86400 ? 0 : 4;
      return t.state === 'sin_fecha' ? 2 : t.state === 'sin_sincronizar' ? 3 : 1;
    };
    const smart = (a: TaskRow, b: TaskRow) => rank(a) - rank(b) || (rank(a) === 1 ? dueAsc(a, b) : dueDesc(a, b));
    return [...f].sort(sort === 'due' && group === 'todo' ? smart : byFn[sort]);
  });

  // ── Tablero ──
  const staleCount = $derived(filtered.filter(stale).length);
  const cols = $derived.by<Col[]>(() => {
    const live = filtered.filter((t) => showStale || !stale(t));
    if (by === 'priority') {
      const todo = live.filter((t) => todoStates.includes(t.state) && !stale(t));
      return [3, 2, 1, 0].map((p) => ({ id: String(p), title: ['Sin prioridad', 'Prioridad baja', 'Prioridad media', 'Prioridad alta'][p], color: `var(--c-${['muted', 'blue', 'amber', 'red'][p]})`, items: todo.filter((t) => t.priority === p).sort(dueAsc) }));
    }
    if (by === 'course') {
      return courses.filter(([id]) => !courseId || String(id) === courseId)
        .map(([id, name]) => ({ id: String(id), title: name, color: `hsl(${hue(id)} 70% 55%)`, items: live.filter((t) => t.course_id === id).sort((a, b) => order[a.state] - order[b.state] || dueAsc(a, b)) }))
        .filter((c) => c.items.length);
    }
    const defs: [string, string, string, string | undefined, (a: TaskRow, b: TaskRow) => number][] = [
      ['late', 'Vencidas', 'red', undefined, dueDesc],
      ['todo', 'Por hacer', 'blue', undefined, dueAsc],
      ['doing', 'En curso', 'violet', undefined, dueAsc],
      ['done', 'Hechas', 'amber', 'Marcadas por ti; no enviadas a Moodle', dueDesc],
      ['nodate', 'Sin fecha', 'muted', undefined, (a, b) => a.name.localeCompare(b.name)],
      ['sent', 'Entregadas', 'green', undefined, dueDesc],
      ['graded', 'Evaluadas', 'teal', undefined, dueDesc],
    ];
    const out: Col[] = defs.map(([id, title, tone, hint, s]) => ({ id, title, hint, color: `var(--c-${tone})`, items: live.filter((t) => stateCol(t) === id).sort(s) }));
    if (showStale) out.push({ id: 'stale', title: 'Antiguas', color: 'var(--c-muted)', hint: 'Vencidas hace más de 30 días', items: live.filter((t) => stateCol(t) === 'stale').sort(dueDesc) });
    return out;
  });
  // Columna del tablero por estado: lo que dice Moodle, salvo en tareas sin entregar, donde tu columna personal (local) manda.
  const naturalCol = (t: TaskRow) => {
    switch (t.state) {
      case 'entregada': return 'sent';
      case 'evaluada': return 'graded';
      case 'iniciada': case 'preparada': return 'doing';
      case 'pendiente': return 'todo';
      case 'sin_fecha': case 'sin_sincronizar': return 'nodate';
      default: return stale(t) ? 'stale' : 'late';
    }
  };
  const movable = (t: TaskRow) => todoStates.includes(t.state);
  const stateCol = (t: TaskRow) => movable(t) && t.done_local ? 'done' : movable(t) && (t.local_status === 'todo' || t.local_status === 'doing') ? t.local_status : naturalCol(t);
  const dropCols = ['todo', 'doing', 'done'];
  const boardTotal = $derived(cols.reduce((n, c) => n + c.items.length, 0));

  // Arrastre con eventos de puntero (HTML5 drag&drop no es fiable en todos los WebView). Tablero por prioridad y, para tareas sin entregar, por estado (columna personal local).
  let drag = $state<{ t: TaskRow; x: number; y: number; w: number; over: string | null } | null>(null);
  let down: { t: TaskRow; sx: number; sy: number; w: number; ox: number; oy: number } | null = null;
  let justDragged = false;
  const colAt = (x: number, y: number) => (document.elementFromPoint(x, y)?.closest('[data-col]') as HTMLElement | null)?.dataset.col ?? null;
  function pdown(e: PointerEvent, t: TaskRow) {
    if (e.button !== 0 || !(by === 'priority' || (by === 'state' && movable(t)))) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    down = { t, sx: e.clientX, sy: e.clientY, w: r.width, ox: e.clientX - r.left, oy: e.clientY - r.top };
    window.addEventListener('pointermove', pmove); window.addEventListener('pointerup', pup, { once: true }); window.addEventListener('pointercancel', pup, { once: true });
  }
  function pmove(e: PointerEvent) {
    if (!down) return;
    if (!drag && Math.hypot(e.clientX - down.sx, e.clientY - down.sy) < 6) return;
    drag = { t: down.t, x: e.clientX - down.ox, y: e.clientY - down.oy, w: down.w, over: colAt(e.clientX, e.clientY) };
  }
  async function pup() {
    window.removeEventListener('pointermove', pmove); window.removeEventListener('pointercancel', pup);
    const d = drag; down = null; drag = null;
    if (!d) return;
    justDragged = true; setTimeout(() => (justDragged = false), 0);
    if (d.over === null) return;
    if (by === 'priority') { if (Number(d.over) !== d.t.priority) await flag(d.t, { priority: Number(d.over) }); return; }
    if (by !== 'state' || !dropCols.includes(d.over) || stateCol(d.t) === d.over) return;
    // Si el destino es la columna natural de Moodle se borra la marca local; si no, se guarda tu columna.
    if (d.over === 'done') await flag(d.t, { doneLocal: true, localStatus: '' });
    else await flag(d.t, { doneLocal: false, localStatus: naturalCol(d.t) === d.over ? '' : (d.over as 'todo' | 'doing') });
  }
  const cardClick = (t: TaskRow) => { if (!justDragged) toggle(t); };

  // «Urgente» solo por fecha límite cercana y sin entregar; nunca porque la tarea sea reciente.
  const urgent = (t: TaskRow) => todoStates.includes(t.state) && t.seconds_to_due !== null && t.seconds_to_due >= 0 && t.seconds_to_due < 48 * 3600;

  async function flag(t: TaskRow, f: { priority?: number; favorite?: boolean; doneLocal?: boolean; localStatus?: '' | 'todo' | 'doing' }) { await taskSetFlags(t.id, f).catch((e) => (error = String(e))); await load(); }
  function toggle(t: TaskRow) { openId = openId === t.id ? null : t.id; error = ''; }
  const openTask = $derived(openId === null ? null : tasks.find((t) => t.id === openId) ?? null);
</script>



{#snippet cardBody(t: TaskRow)}
  {@const si = stateInfo[t.state]}
  <div class="kc-top"><i class="dot" style="background: hsl({hue(t.course_id)} 70% 55%)"></i><span class="kc-course">{t.course_name}</span>{#if t.favorite}<Icon name="star" size={13} />{/if}</div>
  <b class="kc-title">{t.name}</b>
  <div class="kc-meta">
    {#if t.state === 'sin_sincronizar'}<span class="kc-due muted">Sin datos de Moodle</span>
    {:else if t.due_at}<span class="kc-due" class:warn={urgent(t)} class:bad={t.state === 'fuera_de_plazo'}><Icon name="clock" size={12} />{fmtDue(t.due_at)}{#if todoStates.includes(t.state)}<em>{fmtRemaining(t.seconds_to_due)}</em>{/if}</span>
    {:else}<span class="kc-due muted">Sin fecha límite</span>{/if}
  </div>
  <div class="kc-foot">
    {#if by !== 'state'}<span class="badge b-{si.tone}">{stale(t) ? 'Antigua' : si.label}</span>{/if}
    {#if t.grade}<span class="grade">{t.grade}</span>{/if}
    {#if t.priority > 0 && by !== 'priority'}<span class="kc-prio" title="Prioridad personal">{'●'.repeat(t.priority)}</span>{/if}
    {#if t.local_files}<span class="kc-files" title="Archivos descargados"><Icon name="file" size={12} />{t.local_files}</span>{/if}
  </div>
{/snippet}

<h1>Tareas y actividades</h1>
<p class="muted" style="margin:4px 0 0">Estado y fecha límite de cada tarea, leídos de Moodle. Nada se envía desde aquí sin tu confirmación.</p>
{#if error && openId === null}<p class="msg err" role="alert">{error}</p>{/if}

<div class="bar">
  {#if view === 'list'}
    <div class="chips" role="group" aria-label="Grupo">
      <button class="chip" class:on={group === 'todo'} onclick={() => (group = 'todo')}>Por hacer <i>{counts.todo}</i></button>
      <button class="chip" class:on={group === 'done'} onclick={() => (group = 'done')}>Entregadas y evaluadas <i>{counts.done}</i></button>
      <button class="chip" class:on={group === 'all'} onclick={() => (group = 'all')}>Todas <i>{tasks.length}</i></button>
    </div>
  {:else}
    <div class="seg" role="group" aria-label="Agrupar el tablero por">
      <button class:on={by === 'state'} onclick={() => setBy('state')}>Estado</button>
      <button class:on={by === 'course'} onclick={() => setBy('course')}>Asignatura</button>
      <button class:on={by === 'priority'} onclick={() => setBy('priority')}>Prioridad</button>
    </div>
    {#if staleCount}<button class="chip" class:on={showStale} onclick={() => (showStale = !showStale)}>Antiguas <i>{staleCount}</i></button>{/if}
  {/if}
  {#if counts.late}<span class="late"><Icon name="alert" size={15} />{counts.late} vencida(s) recientemente sin entregar</span>{/if}
  <div class="seg view" role="group" aria-label="Vista">
    <button class:on={view === 'list'} onclick={() => setView('list')}><Icon name="tasks" size={14} /> Lista</button>
    <button class:on={view === 'board'} onclick={() => setView('board')}><Icon name="dashboard" size={14} /> Tablero</button>
  </div>
</div>
<div class="bar">
  <input type="search" placeholder="Buscar tarea o asignatura…" bind:value={q} aria-label="Buscar" />
  <select bind:value={courseId} aria-label="Asignatura"><option value="">Todas las asignaturas</option>{#each courses as [id, n]}<option value={String(id)}>{n}</option>{/each}</select>
  {#if view === 'list'}<select bind:value={sort} aria-label="Ordenar por"><option value="due">Orden: fecha límite</option><option value="priority">Orden: prioridad</option><option value="course">Orden: asignatura</option><option value="state">Orden: estado</option></select>{/if}
</div>

{#if loaded && tasks.length === 0}
  <div class="card empty"><span class="ico tone-blue"><Icon name="tasks" size={24} /></span><h2>No hay tareas guardadas</h2><p class="muted">Sincroniza en «Moodle / Conexión»: se leen las tareas de cada curso con su estado y fecha límite.</p></div>
{:else if view === 'list'}
  {#if loaded && shown.length === 0}<p class="muted">Ninguna tarea con estos filtros.</p>{/if}
  <ul class="list">
    {#each shown as t, i (t.id)}
      {@const si = stateInfo[t.state]}
      <li id="task-{t.id}" style="--i:{i}" class="task s-{si.tone}" class:open={openId === t.id} class:stale={stale(t)}>
        <div class="row">
          <button class="star" title="Prioridad personal (clic para cambiar)" aria-label={`Prioridad ${t.priority} de 3`} onclick={() => flag(t, { priority: (t.priority + 1) % 4 })}>{'●'.repeat(t.priority)}{'○'.repeat(3 - t.priority)}</button>
          <button class="main" onclick={() => toggle(t)} aria-expanded={openId === t.id}>
            <span class="title">{t.favorite ? '★ ' : ''}{t.name}</span>
            <span class="muted sub">{t.course_name}{t.section_position !== null ? ` · ${t.section_position === 0 ? 'General' : 'Tema ' + t.section_position}` : ''}</span>
          </button>
          <div class="due">
            <span>{t.state === 'sin_sincronizar' ? 'Sin datos de Moodle' : fmtDue(t.due_at)}</span>
            {#if t.due_at && todoStates.includes(t.state)}<small class:warn={urgent(t)} class:bad={t.state === 'fuera_de_plazo'}>{urgent(t) ? 'Urgente · ' : ''}{fmtRemaining(t.seconds_to_due)}</small>{/if}
          </div>
          <span class="badge b-{si.tone}">{stale(t) ? 'Antigua · sin entregar' : si.label}</span>
          {#if t.grade}<span class="grade" title="Calificación en Moodle">{t.grade}</span>{/if}
        </div>
        {#if openId === t.id}<div class="detail"><TaskDetail {t} onChange={load} /></div>{/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="hint muted small">
    {#if by === 'priority'}Arrastra las tarjetas entre columnas para cambiar tu prioridad personal (se guarda solo en tu equipo).
    {:else if by === 'state'}Arrastra las tareas sin entregar entre «Por hacer», «En curso» y «Hechas» para organizarte: se guarda solo en tu equipo y no cambia nada en Moodle. Entregadas y Evaluadas vienen de Moodle y no se mueven.
    {:else}Las columnas reflejan la asignatura; para mover tarjetas usa el tablero por estado o por prioridad. Pulsa una para ver su detalle.{/if}
    · {boardTotal} {boardTotal === 1 ? 'tarea' : 'tareas'}
  </p>
  <div class="board" class:dragging={!!drag} role="list">
    {#each cols as c (c.id)}
      <section class="col" class:over={drag?.over === c.id} data-col={by === 'priority' || (by === 'state' && dropCols.includes(c.id)) ? c.id : undefined} style="--t: {c.color}" role="listitem">
        <header class="col-h"><i class="col-dot"></i><h2>{c.title}</h2><span class="col-n">{c.items.length}</span></header>
        {#if c.hint}<p class="col-hint muted small">{c.hint}</p>{/if}
        <div class="col-body">
          {#each c.items as t, i (t.id)}
            {@const si = stateInfo[t.state]}
            <div id="task-{t.id}" class="kc s-{si.tone}" class:ghost={drag?.t.id === t.id} class:open={openId === t.id} class:stale={stale(t)} style="--i:{i}"
              role="button" tabindex="0" onpointerdown={(e) => pdown(e, t)} onclick={() => cardClick(t)} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggle(t); } }}>
              {@render cardBody(t)}
            </div>
          {:else}
            <div class="col-empty">{by === 'priority' || (by === 'state' && dropCols.includes(c.id)) ? 'Suelta aquí una tarea' : 'Sin tareas'}</div>
          {/each}
        </div>
      </section>
    {/each}
  </div>
{/if}

{#if drag}
  <div class="kc float s-{stateInfo[drag.t.state].tone}" style="left: {drag.x}px; top: {drag.y}px; width: {drag.w}px">{@render cardBody(drag.t)}</div>
{/if}

{#if view === 'board' && openTask}
  <TaskDrawer task={openTask} onClose={() => (openId = null)} onChange={load} />
{/if}

<style>
  .bar { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin: 14px 0; }
  .bar input[type='search'] { flex: 1; min-width: 220px; }
  .view { margin-left: auto; } .view button { display: inline-flex; align-items: center; gap: 6px; }
  .late { color: var(--bad); font-size: 13px; font-weight: 600; display: inline-flex; align-items: center; gap: 6px; }
  .hint { margin: 0 0 12px; }

  /* ── Lista ── */
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  .task { --t: var(--c-muted); background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius-lg); box-shadow: var(--shadow); overflow: hidden; position: relative; animation: rise .45s var(--ease) backwards; animation-delay: calc(min(var(--i, 0), 12) * 30ms); transition: border-color .18s; }
  .task::before { content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: var(--t); }
  .task:hover { border-color: var(--line-strong); } .task.open { border-color: var(--accent-line); }
  .s-red { --t: var(--c-red); } .s-green { --t: var(--c-green); } .s-blue { --t: var(--c-blue); } .s-violet { --t: var(--c-violet); } .s-teal { --t: var(--c-teal); } .s-amber { --t: var(--c-amber); }
  .row { display: grid; grid-template-columns: auto 1fr auto auto auto; gap: 14px; align-items: center; padding: 13px 18px 13px 20px; }
  .star { background: none; border: 0; color: var(--warn); letter-spacing: 2px; font-size: 11px; transition: transform .2s var(--ease-spring); } .star:hover { transform: scale(1.25); }
  .main { background: none; border: 0; text-align: left; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .title { font-weight: 600; overflow-wrap: anywhere; font-size: 14.5px; } .sub { font-size: 12.5px; }
  .due { display: flex; flex-direction: column; align-items: flex-end; font-size: 13px; white-space: nowrap; } .due small { font-size: 12px; color: var(--muted); } .due small.warn { color: var(--warn); font-weight: 700; } .due small.bad { color: var(--bad); font-weight: 600; }
  .badge { --t: var(--t-badge, var(--c-muted)); } .b-green { --t-badge: var(--c-green); } .b-red { --t-badge: var(--c-red); } .b-teal { --t-badge: var(--c-teal); } .b-violet { --t-badge: var(--c-violet); } .b-blue { --t-badge: var(--c-blue); } .b-muted { --t-badge: var(--c-muted); }
  .grade { font-size: 12px; font-weight: 700; color: var(--ok); white-space: nowrap; padding: 2px 9px; border-radius: 4px; background: color-mix(in srgb, var(--ok) 14%, transparent); border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent); }
  .detail { padding: 6px 22px 20px 24px; border-top: 1px solid var(--line); animation: rise .3s var(--ease) backwards; }
  .task.stale { opacity: .55; } .task.stale:hover { opacity: .9; }
  .badge, .grade { justify-self: start; }
  @media (max-width: 900px) { .row { grid-template-columns: auto 1fr; } .due, .badge, .grade { grid-column: 2; justify-self: start; align-items: flex-start; } }

  /* ── Tablero ── */
  .board { display: flex; gap: 14px; overflow-x: auto; align-items: stretch; height: calc(100vh - 330px); min-height: 440px; padding: 2px 2px 12px; }
  .board.dragging { cursor: grabbing; user-select: none; -webkit-user-select: none; }
  .col { flex: 0 0 300px; min-height: 0; display: flex; flex-direction: column; background: var(--panel-2); border: 1px solid var(--line); border-top: 3px solid var(--t); border-radius: var(--radius-lg); transition: background .15s, border-color .15s, box-shadow .15s; animation: rise .45s var(--ease) backwards; }
  .col.over { background: color-mix(in srgb, var(--t) 12%, var(--panel-2)); border-color: var(--t); box-shadow: 0 0 0 2px color-mix(in srgb, var(--t) 40%, transparent); }
  .col-h { display: flex; align-items: center; gap: 9px; padding: 12px 14px 8px; }
  .col-dot { width: 9px; height: 9px; border-radius: 50%; background: var(--t); flex: none; }
  .col-h h2 { font-family: var(--font-display); font-size: 14.5px; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-n { font: 700 11.5px/1 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; padding: 3px 8px; border-radius: 99px; background: var(--panel-3); color: var(--muted); }
  .col-hint { margin: 0 14px 6px; }
  .col-body { flex: 1; min-height: 0; overflow-y: auto; padding: 4px 10px 12px; display: flex; flex-direction: column; gap: 9px; }
  .col-empty { border: 1px dashed var(--line-strong); border-radius: var(--radius); padding: 18px 10px; text-align: center; color: var(--faint); font-size: 12.5px; }

  .kc { --t: var(--c-muted); display: flex; flex-direction: column; gap: 7px; padding: 11px 12px 11px 14px; position: relative; text-align: left; cursor: pointer; background: var(--panel-solid); border: 1px solid var(--line); border-radius: var(--radius); box-shadow: var(--shadow);
    animation: rise .4s var(--ease) backwards; animation-delay: calc(min(var(--i, 0), 10) * 30ms); transition: transform .15s var(--ease), border-color .15s, box-shadow .15s, opacity .15s; touch-action: none; }
  .kc::before { content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: var(--t); border-radius: var(--radius) 0 0 var(--radius); }
  .kc:hover { border-color: var(--accent-line); transform: translateY(-1px); }
  .kc.open { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .kc.stale { opacity: .6; } .kc.ghost { opacity: .3; }
  .kc.float { position: fixed; z-index: 300; pointer-events: none; cursor: grabbing; animation: none; transform: rotate(2.5deg) scale(1.03); box-shadow: var(--shadow-lg); border-color: var(--accent); }
  .kc-top { display: flex; align-items: center; gap: 7px; font-size: 11.5px; color: var(--muted); } .kc-top :global(svg) { color: var(--warn); fill: var(--warn); margin-left: auto; flex: none; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; } .kc-course { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 550; }
  .kc-title { font-size: 13.5px; line-height: 1.35; font-weight: 600; overflow-wrap: anywhere; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .kc-meta { font-size: 12px; } .kc-due { display: inline-flex; align-items: center; gap: 5px; flex-wrap: wrap; color: var(--muted); } .kc-due em { font-style: normal; opacity: .85; margin-left: 2px; }
  .kc-due.warn { color: var(--warn); font-weight: 700; } .kc-due.bad { color: var(--bad); font-weight: 600; }
  .kc-foot { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; } .kc-foot:empty { display: none; }
  .kc-prio { color: var(--warn); font-size: 9px; letter-spacing: 2px; } .kc-files { display: inline-flex; align-items: center; gap: 3px; font-size: 11.5px; color: var(--muted); margin-left: auto; }
  .kc .badge { font-size: 10.5px; padding: 1px 7px; } .kc .grade { font-size: 11px; padding: 1px 7px; }
</style>
