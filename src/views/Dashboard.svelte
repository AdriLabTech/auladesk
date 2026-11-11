<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount } from 'svelte';
  import { dashboardData, moodleStatus, getSetting, setSetting, familiasOverview, syncAll, type PlatformOutcome, type DashboardData, type Overview, type SessionState } from '../lib/api';
  import { fam, refreshFamilias } from '../lib/familias.svelte';
  import { greeting, fmtAgo, fmtDue, fmtTime, dayLabel, eventKinds } from '../lib/format';
  import { go } from '../lib/nav.svelte';
  import Icon from '../lib/Icon.svelte';
  import Counter from '../lib/Counter.svelte';
  import DashHero from '../lib/DashHero.svelte';

  let d = $state<DashboardData | null>(null);
  let error = $state('');
  let ready = $state(false); // activa las transiciones de barras y anillo tras el primer pintado

  // Filtros por plataforma (se recuerdan): cada elemento del panel dice de qué plataforma viene.
  let hidden = $state<string[]>([]);
  let moodleSession = $state<SessionState>('unknown');
  let famData = $state<{ id: number; name: string; o: Overview }[]>([]);
  const showMoodle = $derived(!hidden.includes('moodle'));
  const showFam = $derived(!hidden.includes('familias'));
  let syncing = $state(false); let outcomes = $state<PlatformOutcome[]>([]);
  async function syncEverything() { syncing = true; outcomes = []; try { outcomes = await syncAll(); } catch (e) { error = String(e); } finally { syncing = false; load(); } }
  async function toggle(p: string) { hidden = hidden.includes(p) ? hidden.filter((x) => x !== p) : [...hidden, p]; try { await setSetting('dash_filters', hidden.join(',')); } catch (e) { console.error(e); } }
  const srcVisible = (src: string) => !(src === 'moodle' && !showMoodle) && !(src === 'familias' && !showFam);
  const srcLabel: Record<string, string> = $derived({ moodle: 'Moodle', familias: tt('Familias'), personal: tt('Personal') });

  async function load() {
    try { moodleSession = await moodleStatus(); await refreshFamilias(); famData = await Promise.all((fam.status?.students ?? []).map(async (s) => ({ id: s.id, name: s.display_name, o: await familiasOverview(s.id) }))); } catch (e) { console.error(e); }
    try { d = await dashboardData(); error = ''; } catch (e) { error = String(e); }
    requestAnimationFrame(() => requestAnimationFrame(() => (ready = true)));
  }
  onMount(() => { getSetting('dash_filters').then((v) => { hidden = v ? v.split(',').filter(Boolean) : []; }).catch(() => {}); load(); const t = setInterval(load, 60000); return () => clearInterval(t); });

  const doneN = $derived(d ? d.counts.submitted + d.counts.graded : 0);
  const pct = $derived(d && doneN + d.counts.todo > 0 ? Math.round((doneN / (doneN + d.counts.todo)) * 100) : 0);

  const tone = (score: number) => (score >= 90 ? 'red' : score >= 70 ? 'amber' : score >= 50 ? 'blue' : 'muted');
  const urgencyLabel = (score: number) => (score >= 100 ? 'Vencida' : score >= 90 ? 'Urgente' : score >= 70 ? 'Esta semana' : score >= 50 ? 'Próxima' : 'Sin prisa');

  const groupedEvents = $derived.by(() => {
    const m = new Map<string, NonNullable<typeof d>['events']>();
    for (const e of (d?.events ?? []).filter((x) => srcVisible(x.source))) { const k = dayLabel(e.starts_at); (m.get(k) ?? m.set(k, []).get(k)!).push(e); }
    return [...m.entries()];
  });
  const feed: Record<string, { icon: string; tone: string }> = {
    new_activity: { icon: 'plus', tone: 'blue' }, date_changed: { icon: 'refresh', tone: 'amber' }, grade: { icon: 'star', tone: 'violet' },
    submission: { icon: 'check', tone: 'green' }, download: { icon: 'download', tone: 'teal' },
  };
  const courses = $derived([...(d?.per_course ?? [])].sort((a, b) => b.todo - a.todo || a.name.localeCompare(b.name)));
</script>

<div class="dash">
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}

  <div class="filters" role="group" aria-label={tt('Plataformas')}>
    <button class="chip" class:on={showMoodle} aria-pressed={showMoodle} onclick={() => toggle('moodle')}>{tt('Moodle')}</button>
    <button class="chip" class:on={showFam} aria-pressed={showFam} onclick={() => toggle('familias')}>{tt('Familias')}</button>
    <button class="btn sm sync" disabled={syncing} onclick={syncEverything} title={tt('Sincroniza Moodle y Familias a la vez')}>{#if !syncing}<Icon name="refresh" size={14} />{/if}{syncing ? 'Sincronizando…' : 'Sincronizar todo'}</button>
  </div>
  {#if outcomes.length}
    <ul class="outc" role="status">{#each outcomes as o}<li class:bad={!o.ok}><b>{o.label}</b> · {tt(o.ok ? 'listo' : 'no se pudo')} ({(o.millis / 1000).toFixed(1)} s)<small>{o.message}</small></li>{/each}</ul>
  {/if}

  {#if !d && !error}
    <div class="skeleton" style="height:150px"></div>
    <div class="kpis" style="margin-top:16px">{#each [0, 1, 2, 3] as _}<div class="skeleton" style="height:104px"></div>{/each}</div>
  {:else if d}
    {#if d.total_tasks === 0 && famData.length === 0}
      <section class="card empty glow">
        <span class="ico tone-blue"><Icon name="sparkles" size={26} /></span>
        <h1>{tt('Bienvenido a AulaDesk')}</h1>
        <p class="muted">{tt('Aún no hay datos. Inicia sesión en Moodle y sincroniza: tus tareas, fechas y archivos aparecerán aquí.')}</p>
        <button class="btn primary" onclick={() => go('moodle')}><Icon name="plug" size={16} /> {tt('Moodle · Conexión')}</button>
        <button class="btn" onclick={() => go('f_conn')}><Icon name="cap" size={16} /> {tt('Familias · Conexión')}</button>
      </section>
    {:else}
      <div class="stagger">
        {#if showFam && famData.length}
          <section class="fam">
            <h2 class="plath"><span class="plat">{tt('Familias')}</span>{#if fam.status && fam.status.session !== 'active'}<small class="stale">{tt('Sin sesión')} · {tt('datos de')} {fam.status.last_sync_at ? fmtAgo(fam.status.last_sync_at) : tt('ninguna sincronización')}</small>{/if}</h2>
            <div class="famgrid">
              {#each famData as f (f.id)}
                <button class="card fcard" onclick={() => { fam.activeId = f.id; go('f_home'); }}>
                  <b>{f.name}</b>
                  <span class="fk"><span><b>{f.o.average !== null ? f.o.average.toFixed(1) : '—'}</b><small>{tt('media')}</small></span><span><b>{f.o.unjustified}</b><small>{tt('faltas sin justificar')}</small></span><span><b>{f.o.unread}</b><small>{tt('sin leer')}</small></span></span>
                </button>
              {/each}
            </div>
          </section>
        {/if}
        {#if showMoodle && d.total_tasks > 0}
        {#if moodleSession !== 'active'}<p class="stale-note" role="status"><Icon name="alert" size={15} /> {tt('Moodle sin sesión: se muestran los datos de la última sincronización')}{d.last_sync_at ? ` (${fmtAgo(d.last_sync_at)})` : ''}, {tt('no la situación actual.')}</p>{/if}
        <DashHero todo={d.counts.todo} overdue={d.counts.overdue_recent} topName={d.priority[0]?.name ?? null} topReason={d.priority[0]?.reason ?? null} {pct} lastSyncAt={d.last_sync_at} {ready} onTasks={() => go('tasks')} onSync={() => go('moodle')} />

        <section class="kpis">
          <button class="kpi glow tone-amber" onclick={() => go('tasks')}>
            <span class="ico"><Icon name="flame" /></span>
            <span class="kv"><Counter value={d.counts.todo} /></span><span class="kl">{tt('Por hacer')}</span>
            <small class:bad={d.counts.overdue_recent > 0}>{d.counts.overdue_recent ? tt(d.counts.overdue_recent === 1 ? '{n} vencida' : '{n} vencidas', { n: d.counts.overdue_recent }) : d.counts.undated ? tt('{n} sin fecha', { n: d.counts.undated }) : tt('al día')}</small>
          </button>
          <button class="kpi glow tone-green" onclick={() => go('submissions')}>
            <span class="ico"><Icon name="submissions" /></span>
            <span class="kv"><Counter value={d.counts.submitted} /></span><span class="kl">{tt('Entregadas')}</span><small>{tt('esperando nota')}</small>
          </button>
          <button class="kpi glow tone-violet" onclick={() => go('tasks')}>
            <span class="ico"><Icon name="star" /></span>
            <span class="kv"><Counter value={d.counts.graded} /></span><span class="kl">{tt('Evaluadas')}</span><small>{tt('con calificación')}</small>
          </button>
          <button class="kpi glow tone-red" onclick={() => go('calendar')}>
            <span class="ico"><Icon name="calendar" /></span>
            <span class="kv"><Counter value={d.counts.exams_30d} /></span><span class="kl">{tt('Exámenes')}</span><small>{tt('en 30 días')}</small>
          </button>
        </section>

        <div class="cols">
          <section class="card prio">
            <header><h2><Icon name="zap" size={16} /> {tt('Prioridades')}</h2><span class="muted small">{tt('puntuación por fecha, estado y tu prioridad')}</span></header>
            <ol>
              {#each d.priority as p, i (p.id)}
                <li style="--i:{i}">
                  <button class="p glow tone-{tone(p.score)}" onclick={() => go('tasks', { taskId: p.id })}>
                    <span class="rank">{i + 1}</span>
                    <span class="pt"><b>{p.name}</b><small class="muted">{p.course}</small><small class="why">{p.reason}</small></span>
                    <span class="pr"><span class="badge b-{tone(p.score)}">{urgencyLabel(p.score)}</span><small class="muted">{p.due_at ? fmtDue(p.due_at) : 'Sin fecha límite'}</small></span>
                    <span class="go"><Icon name="right" size={16} /></span>
                  </button>
                </li>
              {:else}
                <li class="empty-li muted"><Icon name="check" size={18} /> {tt('Nada pendiente. Todo entregado.')}</li>
              {/each}
            </ol>
            {#if d.counts.stale > 0}<p class="muted small foot">{tt(d.counts.stale === 1 ? '{n} tarea antigua (vencida hace más de 30 días) no se cuenta aquí.' : '{n} tareas antiguas (vencidas hace más de 30 días) no se cuentan aquí.', { n: d.counts.stale })}</p>{/if}
          </section>

          <section class="card up">
            <header><h2><Icon name="calendar" size={16} /> {tt('Próximos 14 días')}</h2><button class="btn ghost sm" onclick={() => go('calendar')}>Calendario <Icon name="right" size={14} /></button></header>
            {#each groupedEvents as [label, list] (label)}
              <h3 class="day" class:now={label === 'Hoy'}>{label}</h3>
              {#each list as e (e.id)}
                {@const k = eventKinds[e.kind]}
                <div class="ev tone-{k?.tone ?? 'muted'}">
                  <i class="bar-v"></i>
                  <span class="time">{e.all_day ? 'Todo el día' : fmtTime(e.starts_at)}</span>
                  <span class="et"><b>{e.title}</b><small class="muted">{srcLabel[e.source] ?? e.source} · {k?.label ?? e.kind}{e.course_name ? ` · ${e.course_name}` : ''}{e.student_name ? ` · ${e.student_name}` : ''}</small></span>
                </div>
              {/each}
            {:else}
              <p class="muted"><Icon name="calendar" size={16} /> {tt('Sin eventos en los próximos 14 días.')}</p>
            {/each}
          </section>

          <section class="card courses">
            <header><h2><Icon name="courses" size={16} /> {tt('Progreso por asignatura')}</h2></header>
            {#each courses as c, i (c.course_id)}
              {@const tot = c.done + c.todo}
              <button class="cb" onclick={() => go('courses', { courseId: c.course_id })}>
                <span class="cn">{c.name}</span>
                <span class="cm">{c.done}/{tot}</span>
                <span class="track"><span class="fill" style="width: {ready ? (tot ? (c.done / tot) * 100 : 0) : 0}%; transition-delay: {i * 70}ms"></span></span>
              </button>
            {:else}<p class="muted">{tt('Sin tareas por asignatura todavía.')}</p>{/each}
          </section>

          <section class="card recent">
            <header><h2><Icon name="sparkles" size={16} /> {tt('Actividad reciente')}</h2></header>
            {#each d.recent as r}
              {@const f = feed[r.kind] ?? { icon: 'file', tone: 'muted' }}
              <div class="rc tone-{f.tone}">
                <span class="ico"><Icon name={f.icon} size={15} /></span>
                <span class="rt"><b>{r.title}</b>{#if r.detail}<small class="muted">{r.detail}</small>{/if}</span>
                <small class="faint">{fmtAgo(r.at)}</small>
              </div>
            {:else}<p class="muted">{tt('Todavía no hay actividad. Aparecerá tras sincronizar.')}</p>{/each}
          </section>
        </div>
        {:else if showMoodle}<p class="muted">{tt('Moodle: aún sin datos.')} <button class="btn sm" onclick={() => go('moodle')}>{tt('Conexión')}</button></p>{/if}
      </div>
    {/if}
  {/if}
</div>


<style>
  .dash { max-width: 1240px; margin: 0 auto; }
  .stagger { display: flex; flex-direction: column; gap: 16px; }
  .bad { color: var(--bad); }

  .kpis { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 16px; }
  .kpi { text-align: left; display: grid; grid-template-columns: auto 1fr; grid-template-rows: auto auto auto; column-gap: 14px; align-items: center; padding: 18px 20px; background: var(--panel); border: 1px solid var(--line); border-top: 3px solid var(--t); border-radius: var(--radius-lg); box-shadow: var(--shadow); }
  .kpi:hover { transform: translateY(-2px); border-color: var(--accent-line); border-top-color: var(--t); }
  .kpi .ico { grid-row: 1 / 4; width: 46px; height: 46px; }
  .kv { font-family: var(--font-display); font-size: 34px; font-weight: 600; letter-spacing: -.04em; line-height: 1.05; } .kl { font-size: 13px; font-weight: 650; } .kpi small { color: var(--muted); font-size: 12px; }
  .kpi small.bad { color: var(--bad); font-weight: 600; }

  .cols { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr); gap: 16px; align-items: start; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 10px; margin-bottom: 14px; flex-wrap: wrap; }
  header h2 { display: flex; align-items: center; gap: 8px; font-family: var(--font-display); font-size: 17px; } header h2 :global(svg) { color: var(--accent); }

  ol { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  ol li { animation: rise .45s var(--ease) backwards; animation-delay: calc(var(--i) * 55ms + 250ms); }
  .p { width: 100%; display: grid; grid-template-columns: 30px minmax(0, 1fr) auto 20px; gap: 14px; align-items: center; text-align: left; padding: 12px 14px; border-radius: var(--radius); background: var(--panel-2); border: 1px solid var(--line); border-left: 4px solid var(--t); transition: transform .18s var(--ease), background .18s; }
  .p:hover { background: var(--panel-3); transform: translateX(3px); }
  .rank { width: 28px; height: 28px; border-radius: var(--radius); display: grid; place-items: center; font-family: var(--font-display); font-weight: 700; font-size: 14px; color: var(--t); background: color-mix(in srgb, var(--t) 16%, transparent); }
  .pt { display: flex; flex-direction: column; gap: 1px; min-width: 0; } .pt b { overflow-wrap: anywhere; } .pt small { font-size: 12px; } .why { color: var(--t); font-weight: 600; }
  .pr { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; } .pr small { font-size: 11.5px; white-space: nowrap; }
  .go { color: var(--faint); transition: transform .18s var(--ease), color .18s; } .p:hover .go { transform: translateX(3px); color: var(--text); }
  .empty-li { display: flex; gap: 8px; align-items: center; padding: 14px; } .foot { margin: 12px 2px 0; }

  .day { margin: 16px 0 6px; font-size: 11px; text-transform: uppercase; letter-spacing: .1em; color: var(--muted); font-weight: 700; } .day:first-of-type { margin-top: 0; } .day.now { color: var(--accent); }
  .ev { display: grid; grid-template-columns: 3px 64px minmax(0, 1fr); gap: 10px; align-items: stretch; padding: 7px 8px; border-radius: var(--radius); transition: background .15s; } .ev:hover { background: var(--panel-2); }
  .bar-v { background: var(--t); border-radius: 3px; } .time { font-family: 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; font-size: 11.5px; color: var(--muted); padding-top: 2px; }
  .et { display: flex; flex-direction: column; min-width: 0; } .et b { font-weight: 600; overflow-wrap: anywhere; } .et small { font-size: 12px; }

  .cb { width: 100%; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 7px 10px; text-align: left; background: none; border: 0; padding: 9px 8px; border-radius: var(--radius); transition: background .15s; } .cb:hover { background: var(--panel-2); }
  .cn { font-weight: 550; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cm { font-family: 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; font-size: 11.5px; color: var(--muted); }
  .cb .track { grid-column: 1 / -1; height: 6px; border-radius: 99px; background: var(--panel-3); overflow: hidden; stroke: none; }
  .fill { display: block; height: 100%; border-radius: 99px; background: var(--accent); transition: width 1s var(--ease); }

  .rc { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 9px 4px; border-top: 1px solid var(--line); } .rc:first-of-type { border-top: 0; }
  .rc .ico { width: 30px; height: 30px; } .rt { display: flex; flex-direction: column; min-width: 0; } .rt b { font-weight: 600; font-size: 13px; overflow-wrap: anywhere; } .rt small { font-size: 12px; }

  @media (max-width: 1100px) { .kpis { grid-template-columns: repeat(2, 1fr); } .cols { grid-template-columns: 1fr; } }
  @media (max-width: 760px) { .p { grid-template-columns: 30px 1fr; } .pr { grid-column: 2; align-items: flex-start; } .go { display: none; } }
  .sync { margin-left: auto; } .outc { list-style: none; padding: 0; margin: -6px 0 16px; display: grid; gap: 4px; font-size: 13px; } .outc li { display: flex; gap: 8px; flex-wrap: wrap; align-items: baseline; } .outc li.bad { color: var(--warn); } .outc small { color: var(--muted); }
  .filters { display: flex; gap: 8px; flex-wrap: wrap; margin: 0 0 16px; } .chip.off { opacity: .55; cursor: default; }
  .plath { display: flex; align-items: baseline; gap: 12px; margin: 0 0 10px; } .plat { font-size: 11px; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; color: var(--accent); } .stale { color: var(--warn); font-weight: 500; }
  .stale-note { display: flex; gap: 10px; align-items: center; padding: 9px 14px; margin: 0 0 14px; border: 1px solid color-mix(in srgb, var(--warn) 50%, transparent); border-left: 4px solid var(--warn); border-radius: var(--radius); font-size: 13px; background: color-mix(in srgb, var(--warn) 9%, var(--panel)); }
  .fam { margin-bottom: 20px; } .famgrid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 12px; }
  .fcard { display: grid; gap: 10px; text-align: left; color: var(--text); border-left: 4px solid var(--accent); transition: transform .2s var(--ease), border-color .15s; } .fcard:hover { transform: translateY(-2px); border-color: var(--accent-line); }
  .fk { display: flex; gap: 18px; } .fk span { display: flex; flex-direction: column; } .fk b { font: 700 22px var(--font-display); } .fk small { color: var(--muted); font-size: 11.5px; }
</style>