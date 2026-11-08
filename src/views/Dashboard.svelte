<script lang="ts">
  import { onMount } from 'svelte';
  import { dashboardData, type DashboardData } from '../lib/api';
  import { greeting, fmtAgo, fmtDue, fmtTime, dayLabel, eventKinds } from '../lib/format';
  import { go } from '../lib/nav.svelte';
  import Icon from '../lib/Icon.svelte';
  import Counter from '../lib/Counter.svelte';
  import { account, initials } from '../lib/account.svelte';

  let d = $state<DashboardData | null>(null);
  let error = $state('');
  let ready = $state(false); // activa las transiciones de barras y anillo tras el primer pintado

  async function load() {
    try { d = await dashboardData(); error = ''; } catch (e) { error = String(e); }
    requestAnimationFrame(() => requestAnimationFrame(() => (ready = true)));
  }
  onMount(() => { load(); const t = setInterval(load, 60000); return () => clearInterval(t); });

  const doneN = $derived(d ? d.counts.submitted + d.counts.graded : 0);
  const pct = $derived(d && doneN + d.counts.todo > 0 ? Math.round((doneN / (doneN + d.counts.todo)) * 100) : 0);
  const R = 52, C = 2 * Math.PI * R;
  const today = new Date().toLocaleDateString('es-ES', { weekday: 'long', day: 'numeric', month: 'long' });

  const tone = (score: number) => (score >= 90 ? 'red' : score >= 70 ? 'amber' : score >= 50 ? 'blue' : 'muted');
  const urgencyLabel = (score: number) => (score >= 100 ? 'Vencida' : score >= 90 ? 'Urgente' : score >= 70 ? 'Esta semana' : score >= 50 ? 'Próxima' : 'Sin prisa');

  const groupedEvents = $derived.by(() => {
    const m = new Map<string, NonNullable<typeof d>['events']>();
    for (const e of d?.events ?? []) { const k = dayLabel(e.starts_at); (m.get(k) ?? m.set(k, []).get(k)!).push(e); }
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

  {#if !d && !error}
    <div class="skeleton" style="height:150px"></div>
    <div class="kpis" style="margin-top:16px">{#each [0, 1, 2, 3] as _}<div class="skeleton" style="height:104px"></div>{/each}</div>
  {:else if d}
    {#if d.total_tasks === 0}
      <section class="card empty glow">
        <span class="ico tone-blue"><Icon name="sparkles" size={26} /></span>
        <h1>Bienvenido a AulaDesk</h1>
        <p class="muted">Aún no hay datos. Inicia sesión en Moodle y sincroniza: tus tareas, fechas y archivos aparecerán aquí.</p>
        <button class="btn primary" onclick={() => go('moodle')}><Icon name="plug" size={16} /> Ir a Moodle / Conexión</button>
      </section>
    {:else}
      <div class="stagger">
        <section class="hero">
          <div class="hero-txt">
            <span class="eyebrow">{today}</span>
            {#if account.name}<span class="acct" title="Cuenta de Moodle con la sesión iniciada"><i>{initials(account.name)}</i>Sesión de <b>{account.name}</b>{#if account.userId}<small>ID {account.userId}</small>{/if}</span>{/if}
            <h1>{greeting()}.</h1>
            <p class="lead">
              {#if d.counts.todo === 0}No tienes tareas pendientes. Buen momento para adelantar trabajo o repasar.
              {:else}
                Tienes <b>{d.counts.todo}</b> {d.counts.todo === 1 ? 'tarea por hacer' : 'tareas por hacer'}{#if d.counts.overdue_recent}, <b class="bad">{d.counts.overdue_recent}</b> vencida{d.counts.overdue_recent === 1 ? '' : 's'} recientemente{/if}.
                {#if d.priority[0]}Lo primero: <b>{d.priority[0].name}</b>, {d.priority[0].reason}.{/if}
              {/if}
            </p>
            <div class="hero-actions">
              <button class="btn primary" onclick={() => go('tasks')}><Icon name="tasks" size={16} /> Ver tareas</button>
              <button class="btn" onclick={() => go('moodle')}><Icon name="refresh" size={16} /> Sincronizar</button>
              <span class="synced"><i class="pulse-dot" style="--t: {d.last_sync_at ? 'var(--ok)' : 'var(--warn)'}"></i>Sincronizado {fmtAgo(d.last_sync_at)}</span>
            </div>
          </div>
          <div class="ring" role="img" aria-label={`Progreso: ${pct}% de las tareas ya entregadas`}>
            <svg viewBox="0 0 120 120">
                            <circle cx="60" cy="60" r={R} class="track" />
              <circle cx="60" cy="60" r={R} class="bar" stroke-dasharray={C} stroke-dashoffset={ready ? C * (1 - pct / 100) : C} />
            </svg>
            <div class="ring-in"><strong><Counter value={pct} suffix="%" /></strong><small>entregado</small></div>
          </div>
        </section>

        <section class="kpis">
          <button class="kpi glow tone-amber" onclick={() => go('tasks')}>
            <span class="ico"><Icon name="flame" /></span>
            <span class="kv"><Counter value={d.counts.todo} /></span><span class="kl">Por hacer</span>
            <small class:bad={d.counts.overdue_recent > 0}>{d.counts.overdue_recent ? `${d.counts.overdue_recent} vencida${d.counts.overdue_recent === 1 ? '' : 's'}` : d.counts.undated ? `${d.counts.undated} sin fecha` : 'al día'}</small>
          </button>
          <button class="kpi glow tone-green" onclick={() => go('submissions')}>
            <span class="ico"><Icon name="submissions" /></span>
            <span class="kv"><Counter value={d.counts.submitted} /></span><span class="kl">Entregadas</span><small>esperando nota</small>
          </button>
          <button class="kpi glow tone-violet" onclick={() => go('tasks')}>
            <span class="ico"><Icon name="star" /></span>
            <span class="kv"><Counter value={d.counts.graded} /></span><span class="kl">Evaluadas</span><small>con calificación</small>
          </button>
          <button class="kpi glow tone-red" onclick={() => go('calendar')}>
            <span class="ico"><Icon name="calendar" /></span>
            <span class="kv"><Counter value={d.counts.exams_30d} /></span><span class="kl">Exámenes</span><small>en 30 días</small>
          </button>
        </section>

        <div class="cols">
          <section class="card prio">
            <header><h2><Icon name="zap" size={16} /> Prioridades</h2><span class="muted small">puntuación por fecha, estado y tu prioridad</span></header>
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
                <li class="empty-li muted"><Icon name="check" size={18} /> Nada pendiente. Todo entregado.</li>
              {/each}
            </ol>
            {#if d.counts.stale > 0}<p class="muted small foot">{d.counts.stale} tarea{d.counts.stale === 1 ? '' : 's'} antigua{d.counts.stale === 1 ? '' : 's'} (vencidas hace más de 30 días) no se cuentan aquí.</p>{/if}
          </section>

          <section class="card up">
            <header><h2><Icon name="calendar" size={16} /> Próximos 14 días</h2><button class="btn ghost sm" onclick={() => go('calendar')}>Calendario <Icon name="right" size={14} /></button></header>
            {#each groupedEvents as [label, list] (label)}
              <h3 class="day" class:now={label === 'Hoy'}>{label}</h3>
              {#each list as e (e.id)}
                {@const k = eventKinds[e.kind]}
                <div class="ev tone-{k?.tone ?? 'muted'}">
                  <i class="bar-v"></i>
                  <span class="time">{e.all_day ? 'Todo el día' : fmtTime(e.starts_at)}</span>
                  <span class="et"><b>{e.title}</b><small class="muted">{k?.label ?? e.kind}{e.course_name ? ` · ${e.course_name}` : ''}</small></span>
                </div>
              {/each}
            {:else}
              <p class="muted"><Icon name="calendar" size={16} /> Sin eventos en los próximos 14 días.</p>
            {/each}
          </section>

          <section class="card courses">
            <header><h2><Icon name="courses" size={16} /> Progreso por asignatura</h2></header>
            {#each courses as c, i (c.course_id)}
              {@const tot = c.done + c.todo}
              <button class="cb" onclick={() => go('courses', { courseId: c.course_id })}>
                <span class="cn">{c.name}</span>
                <span class="cm">{c.done}/{tot}</span>
                <span class="track"><span class="fill" style="width: {ready ? (tot ? (c.done / tot) * 100 : 0) : 0}%; transition-delay: {i * 70}ms"></span></span>
              </button>
            {:else}<p class="muted">Sin tareas por asignatura todavía.</p>{/each}
          </section>

          <section class="card recent">
            <header><h2><Icon name="sparkles" size={16} /> Actividad reciente</h2></header>
            {#each d.recent as r}
              {@const f = feed[r.kind] ?? { icon: 'file', tone: 'muted' }}
              <div class="rc tone-{f.tone}">
                <span class="ico"><Icon name={f.icon} size={15} /></span>
                <span class="rt"><b>{r.title}</b>{#if r.detail}<small class="muted">{r.detail}</small>{/if}</span>
                <small class="faint">{fmtAgo(r.at)}</small>
              </div>
            {:else}<p class="muted">Todavía no hay actividad. Aparecerá tras sincronizar.</p>{/each}
          </section>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .dash { max-width: 1240px; margin: 0 auto; }
  .stagger { display: flex; flex-direction: column; gap: 16px; }
  .bad { color: var(--bad); }

  /* Cabecera a todo color: el acento del tema es el protagonista */
  .hero { display: flex; align-items: center; justify-content: space-between; gap: 28px; padding: 30px 34px; border-radius: var(--radius-lg); background: var(--accent); color: var(--accent-fg); position: relative; overflow: hidden; }
  .hero::after { content: ''; position: absolute; right: -70px; top: -70px; width: 260px; height: 260px; border-radius: 50%; border: 36px solid color-mix(in srgb, var(--accent-fg) 8%, transparent); pointer-events: none; }
  .hero-txt { min-width: 0; max-width: 680px; position: relative; z-index: 1; }
  .eyebrow { font-size: 12px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; opacity: .72; }
  .acct { display: inline-flex; align-items: center; gap: 9px; margin-left: 14px; padding: 3px 12px 3px 3px; border-radius: 99px; background: color-mix(in srgb, var(--accent-fg) 14%, transparent); font-size: 13px; vertical-align: middle; }
  .acct i { font-style: normal; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; background: var(--accent-fg); color: var(--accent); font: 800 11px/1 var(--font-display); }
  .acct b { font-weight: 700; } .acct small { opacity: .65; font-size: 11.5px; font-family: 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; }
  .hero h1 { font-size: 44px; margin: 6px 0 10px; letter-spacing: -.035em; line-height: 1; }
  .lead { font-size: 15.5px; margin: 0 0 20px; line-height: 1.55; color: color-mix(in srgb, var(--accent-fg) 78%, transparent); } .lead b { color: var(--accent-fg); font-weight: 700; } .lead b.bad { color: var(--accent-fg); text-decoration: underline wavy; text-underline-offset: 3px; }
  .hero-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .hero .btn { background: transparent; border-color: color-mix(in srgb, var(--accent-fg) 40%, transparent); color: var(--accent-fg); } .hero .btn:hover { background: color-mix(in srgb, var(--accent-fg) 10%, transparent); border-color: var(--accent-fg); }
  .hero .btn.primary { background: var(--accent-fg); color: var(--accent); border-color: var(--accent-fg); } .hero .btn.primary:hover { filter: none; opacity: .88; background: var(--accent-fg); }
  .synced { display: inline-flex; gap: 9px; align-items: center; font-size: 12.5px; margin-left: 6px; opacity: .8; }

  .ring { position: relative; width: 170px; height: 170px; flex: none; z-index: 1; }
  .ring svg { width: 100%; height: 100%; transform: rotate(-90deg); }
  .ring circle { fill: none; stroke-width: 10; stroke-linecap: butt; }
  .track { stroke: color-mix(in srgb, var(--accent-fg) 18%, transparent); } .ring .bar { stroke: var(--accent-fg); transition: stroke-dashoffset 1.2s var(--ease) .2s; }
  .ring-in { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; }
  .ring-in strong { font-family: var(--font-display); font-size: 38px; letter-spacing: -.04em; line-height: 1; } .ring-in small { font-size: 12px; margin-top: 4px; opacity: .72; font-weight: 600; }

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
  @media (max-width: 760px) { .hero { flex-direction: column-reverse; align-items: flex-start; padding: 22px; } .ring { width: 130px; height: 130px; } .hero h1 { font-size: 34px; } .p { grid-template-columns: 30px 1fr; } .pr { grid-column: 2; align-items: flex-start; } .go { display: none; } }
</style>
