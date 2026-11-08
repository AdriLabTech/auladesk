<script lang="ts">
  import { onMount } from 'svelte';
  import { calendarList, calendarCreate, calendarUpdate, calendarDelete, calendarSetNotes, type EventRow, type NewEvent } from '../lib/api';
  import Icon from '../lib/Icon.svelte';
  import { eventKinds, personalKinds, fmtTime, fmtDay, startOfDay, addDays, ts, mondayOf, sameDay } from '../lib/format';

  type View = 'month' | 'week' | 'day' | 'agenda';
  let view = $state<View>('month');
  let cursor = $state(startOfDay(new Date()));
  let events = $state<EventRow[]>([]);
  let error = $state('');
  let sel = $state<EventRow | null>(null);
  let notes = $state('');
  let form = $state<null | { id: number | null; title: string; kind: string; date: string; time: string; allDay: boolean; endTime: string; remind: string; notes: string }>(null);
  const today = startOfDay(new Date());

  const range = $derived.by(() => {
    if (view === 'month') { const first = mondayOf(new Date(cursor.getFullYear(), cursor.getMonth(), 1)); return [first, addDays(first, 42)]; }
    if (view === 'week') { const m = mondayOf(cursor); return [m, addDays(m, 7)]; }
    if (view === 'day') return [cursor, addDays(cursor, 1)];
    return [cursor, addDays(cursor, 60)];
  });
  const title = $derived(view === 'month' ? cursor.toLocaleDateString('es-ES', { month: 'long', year: 'numeric' })
    : view === 'week' ? `Semana del ${range[0].toLocaleDateString('es-ES', { day: 'numeric', month: 'short' })}`
    : view === 'day' ? cursor.toLocaleDateString('es-ES', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }) : 'Próximos 60 días');

  async function load() { try { events = await calendarList(ts(range[0]), ts(range[1])); } catch (e) { error = String(e); } }
  onMount(load);
  $effect(() => { range; load(); });

  function move(dir: number) {
    if (view === 'month') cursor = new Date(cursor.getFullYear(), cursor.getMonth() + dir, 1);
    else cursor = addDays(cursor, dir * (view === 'week' ? 7 : view === 'day' ? 1 : 30));
  }
  const dayEvents = (d: Date) => events.filter((e) => sameDay(new Date(e.starts_at * 1000), d));
  const cells = $derived(Array.from({ length: view === 'week' ? 7 : 42 }, (_, i) => addDays(range[0], i)));
  const agenda = $derived.by(() => {
    const m = new Map<string, EventRow[]>();
    for (const e of events) { const k = startOfDay(new Date(e.starts_at * 1000)).toISOString(); (m.get(k) ?? m.set(k, []).get(k)!).push(e); }
    return [...m.entries()];
  });
  const tone = (e: EventRow) => (e.kind === 'due' && e.source === 'moodle' ? 'blue' : eventKinds[e.kind]?.tone ?? 'muted');
  const done = (e: EventRow) => e.activity_state === 'entregada' || e.activity_state === 'evaluada';

  function open(e: EventRow) { sel = e; notes = e.notes ?? ''; form = null; }
  const pad = (n: number) => String(n).padStart(2, '0');
  const dstr = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  function newEvent(d: Date = cursor) { sel = null; form = { id: null, title: '', kind: 'reminder', date: dstr(d), time: '09:00', allDay: false, endTime: '', remind: '', notes: '' }; }
  function editEvent(e: EventRow) {
    const s = new Date(e.starts_at * 1000), en = e.ends_at ? new Date(e.ends_at * 1000) : null;
    form = { id: e.id, title: e.title, kind: e.kind, date: dstr(s), time: `${pad(s.getHours())}:${pad(s.getMinutes())}`, allDay: !!e.all_day, endTime: en ? `${pad(en.getHours())}:${pad(en.getMinutes())}` : '', remind: e.remind_min === null ? '' : String(e.remind_min), notes: e.notes ?? '' };
    sel = null;
  }
  async function save() {
    if (!form) return; error = '';
    const start = new Date(`${form.date}T${form.allDay ? '00:00' : form.time}:00`);
    const end = form.endTime && !form.allDay ? new Date(`${form.date}T${form.endTime}:00`) : null;
    const ev: NewEvent = { title: form.title, kind: form.kind, starts_at: ts(start), ends_at: end ? ts(end) : null, all_day: form.allDay, notes: form.notes || null, remind_min: form.remind === '' ? null : Number(form.remind) };
    try { if (form.id === null) await calendarCreate(ev); else await calendarUpdate(form.id, ev); form = null; await load(); } catch (e) { error = String(e); }
  }
  async function remove(e: EventRow) { try { await calendarDelete(e.id); sel = null; await load(); } catch (x) { error = String(x); } }
  async function saveNotes() { if (sel) { try { await calendarSetNotes(sel.id, notes); await load(); } catch (x) { error = String(x); } } }
</script>

<div class="head">
  <h1>Calendario</h1>
  <div class="tools">
    <div class="seg" role="group" aria-label="Vista">
      {#each [['month', 'Mes'], ['week', 'Semana'], ['day', 'Día'], ['agenda', 'Agenda']] as [v, l]}
        <button class:on={view === v} onclick={() => (view = v as View)}>{l}</button>
      {/each}
    </div>
    <button class="btn sm" onclick={() => move(-1)} aria-label="Anterior"><Icon name="left" size={15} /></button>
    <button class="btn sm" onclick={() => (cursor = startOfDay(new Date()))}>Hoy</button>
    <button class="btn sm" onclick={() => move(1)} aria-label="Siguiente"><Icon name="right" size={15} /></button>
    <button class="btn primary sm" onclick={() => newEvent()}><Icon name="plus" size={15} /> Evento personal</button>
  </div>
</div>
<h2 class="when">{title}</h2>
<p class="legend"><i class="d blue"></i>Entrega <i class="d red"></i>Examen <i class="d violet"></i>Cuestionario <i class="d teal"></i>Curso <i class="d amber"></i>Personal · Los de Moodle no se modifican desde aquí.</p>
{#if error}<p class="msg err" role="alert">{error}</p>{/if}

<div class="layout">
  <div class="main">
    {#if view === 'month' || view === 'week'}
      <div class="grid" class:week={view === 'week'}>
        {#each ['L', 'M', 'X', 'J', 'V', 'S', 'D'] as h}<div class="dow">{h}</div>{/each}
        {#each cells as d}
          {@const ev = dayEvents(d)}
          <div class="cell" class:other={view === 'month' && d.getMonth() !== cursor.getMonth()} class:today={sameDay(d, today)}>
            <button class="num" onclick={() => { cursor = d; view = 'day'; }} aria-label={`Ver el día ${d.getDate()}`}>{d.getDate()}</button>
            {#each ev.slice(0, view === 'week' ? 12 : 3) as e (e.id)}
              <button class="chip t-{tone(e)}" class:cancel={e.cancelled_at} class:done={done(e)} onclick={() => open(e)} title={e.title}>{e.all_day ? '' : fmtTime(e.starts_at) + ' '}{e.title}</button>
            {/each}
            {#if view === 'month' && ev.length > 3}<button class="more" onclick={() => { cursor = d; view = 'day'; }}>+{ev.length - 3} más</button>{/if}
          </div>
        {/each}
      </div>
    {:else if view === 'day'}
      {#each dayEvents(cursor) as e (e.id)}
        <button class="row t-{tone(e)}" class:cancel={e.cancelled_at} onclick={() => open(e)}><b>{e.all_day ? 'Todo el día' : fmtTime(e.starts_at)}</b><span>{e.title}</span><small>{e.course_name ?? (e.source === 'personal' ? 'Personal' : '')}</small></button>
      {:else}<p class="muted">Sin eventos este día.</p>{/each}
    {:else}
      {#each agenda as [k, list] (k)}
        <h3 class="ag">{fmtDay(new Date(k).getTime() / 1000)}</h3>
        {#each list as e (e.id)}
          <button class="row t-{tone(e)}" class:cancel={e.cancelled_at} onclick={() => open(e)}><b>{e.all_day ? 'Todo el día' : fmtTime(e.starts_at)}</b><span>{e.title}</span><small>{e.course_name ?? (e.source === 'personal' ? 'Personal' : '')}</small></button>
        {/each}
      {:else}<p class="muted">No hay eventos en los próximos 60 días.</p>{/each}
    {/if}
  </div>

  {#if sel || form}
    <aside class="card side">
      {#if sel}
        <h2>{sel.title}</h2>
        <p class="muted">{eventKinds[sel.kind]?.label ?? sel.kind} · {sel.source === 'moodle' ? 'Moodle (solo lectura)' : 'Personal'}</p>
        <p>{fmtDay(sel.starts_at)}{sel.all_day ? '' : ', ' + fmtTime(sel.starts_at)}{sel.ends_at ? ' – ' + fmtTime(sel.ends_at) : ''}</p>
        {#if sel.course_name}<p class="muted">{sel.course_name}</p>{/if}
        {#if sel.activity_state}<p>Estado de la tarea: <b>{sel.activity_state.replace('_', ' ')}</b></p>{/if}
        {#if sel.cancelled_at}<p class="warn">Este evento ya no existe en Moodle (se conserva el registro).</p>{/if}
        {#if sel.remind_min !== null}<p class="muted">Aviso {sel.remind_min} min antes</p>{/if}
        <label class="f">Mis notas<textarea rows="3" bind:value={notes} placeholder="Solo en tu equipo"></textarea></label>
        <div class="acts">
          <button class="btn sm" onclick={saveNotes}>Guardar notas</button>
          {#if sel.source === 'personal'}<button class="btn sm" onclick={() => editEvent(sel!)}>Editar</button><button class="btn sm danger" onclick={() => remove(sel!)}>Borrar</button>{/if}
          <button class="btn sm" onclick={() => (sel = null)}>Cerrar</button>
        </div>
      {:else if form}
        <h2>{form.id === null ? 'Nuevo evento personal' : 'Editar evento'}</h2>
        <label class="f">Título<input bind:value={form.title} /></label>
        <label class="f">Tipo<select bind:value={form.kind}>{#each personalKinds as k}<option value={k}>{eventKinds[k].label}</option>{/each}</select></label>
        <label class="f">Fecha<input type="date" bind:value={form.date} /></label>
        <label class="chk"><input type="checkbox" bind:checked={form.allDay} /> Todo el día</label>
        {#if !form.allDay}
          <div class="two"><label class="f">Hora<input type="time" bind:value={form.time} /></label><label class="f">Fin (opcional)<input type="time" bind:value={form.endTime} /></label></div>
          <label class="f">Avisar antes<select bind:value={form.remind}><option value="">Sin aviso</option><option value="0">A la hora</option><option value="15">15 min</option><option value="30">30 min</option><option value="60">1 hora</option><option value="1440">1 día</option></select></label>
        {/if}
        <label class="f">Notas<textarea rows="2" bind:value={form.notes}></textarea></label>
        <div class="acts"><button class="btn primary sm" onclick={save}>Guardar</button><button class="btn sm" onclick={() => (form = null)}>Cancelar</button></div>
      {/if}
    </aside>
  {/if}
</div>

<style>
  .head { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 12px; } .tools { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .when { margin: 12px 0 4px; font-size: 20px; letter-spacing: -.02em; } .when::first-letter { text-transform: uppercase; }
  .legend { color: var(--muted); font-size: 12.5px; margin: 0 0 14px; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 6px; }
  .d { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin: 0 2px 0 12px; box-shadow: 0 0 8px currentColor; }
  .blue { background: var(--c-blue); color: var(--c-blue); } .red { background: var(--c-red); color: var(--c-red); } .violet { background: var(--c-violet); color: var(--c-violet); } .teal { background: var(--c-teal); color: var(--c-teal); } .amber { background: var(--c-amber); color: var(--c-amber); } .green { background: var(--c-green); color: var(--c-green); }
  .layout { display: grid; grid-template-columns: 1fr; gap: 16px; } @media (min-width: 1100px) { .layout:has(.side) { grid-template-columns: 1fr 340px; } }
  .grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 1px; background: var(--line); border: 1px solid var(--line); border-radius: 16px; overflow: hidden; box-shadow: var(--shadow); animation: pop .45s var(--ease) backwards; }
  .dow { background: var(--panel-solid); padding: 8px; text-align: center; font-size: 11.5px; font-weight: 700; letter-spacing: .08em; color: var(--muted); }
  .cell { background: var(--panel-solid); min-height: 100px; padding: 5px; display: flex; flex-direction: column; gap: 3px; min-width: 0; transition: background .18s; } .cell:hover { background: color-mix(in srgb, var(--panel-solid) 88%, var(--accent)); } .grid.week .cell { min-height: 210px; }
  .cell.other { background: color-mix(in srgb, var(--panel-solid) 60%, var(--bg)); } .cell.other .num { opacity: .4; }
  .cell.today { background: color-mix(in srgb, var(--panel-solid) 90%, var(--accent)); } .cell.today .num { background: var(--accent); color: var(--accent-fg); border-radius: 99px; font-weight: 700; }
  .num { align-self: flex-start; background: none; border: 0; font-size: 12px; padding: 2px 7px; color: var(--text); transition: transform .2s var(--ease-spring); } .num:hover { transform: scale(1.15); }
  .chip { border: 0; border-radius: 6px; padding: 2px 6px; font-size: 11.5px; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: block; max-width: 100%; color: var(--text); background: color-mix(in srgb, var(--c) 22%, transparent); box-shadow: inset 2px 0 var(--c); }
  .chip:hover { background: color-mix(in srgb, var(--c) 36%, transparent); transform: translateY(0); }
  .t-blue { --c: var(--c-blue); } .t-red { --c: var(--c-red); } .t-violet { --c: var(--c-violet); } .t-teal { --c: var(--c-teal); } .t-amber { --c: var(--c-amber); } .t-green { --c: var(--c-green); } .t-muted { --c: var(--c-muted); }
  .chip.cancel, .row.cancel { text-decoration: line-through; opacity: .5; } .chip.done { opacity: .55; }
  .more { background: none; border: 0; color: var(--muted); font-size: 11px; text-align: left; padding: 0 4px; }
  .row { display: grid; grid-template-columns: 100px 1fr auto; gap: 14px; align-items: center; width: 100%; border: 1px solid var(--line); border-left: 3px solid var(--c); border-radius: 12px; padding: 11px 14px; margin-bottom: 6px; text-align: left; color: var(--text); background: var(--panel); transition: transform .2s var(--ease), background .2s; animation: rise .4s var(--ease) backwards; }
  .row:hover { transform: translateX(4px); background: var(--panel-3); } .row small { color: var(--muted); }
  .ag { font-size: 12px; text-transform: capitalize; margin: 20px 0 8px; color: var(--accent); font-weight: 700; letter-spacing: .04em; }
  .side { align-self: start; animation: pop .35s var(--ease-spring) backwards; } .side h2 { overflow-wrap: anywhere; font-size: 17px; } .warn { color: var(--warn); }
  .chk { display: flex; gap: 8px; align-items: center; font-size: 13px; margin: 8px 0; } .two { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; } .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 12px; }
</style>
