<script lang="ts">
  import { onMount } from 'svelte';
  import { dashboardData } from './api';
  import { account, initials } from './account.svelte';
  import { greeting } from './format';
  import { t } from './i18n.svelte';
  import { trapFocus } from './a11y';
  import type { WelcomeKind } from './theme.svelte';

  let { kind, onDone }: { kind: Exclude<WelcomeKind, 'reveal' | 'random'>; onDone: () => void } = $props();
  let leaving = $state(false), lines = $state<string[]>([]), stats = $state<{ label: string; bad?: boolean }[]>([]);
  const name = $derived(account.name ?? '');
  const first = $derived(name.split(/\s+/).filter(Boolean)[0] ?? '');
  const HOLD = 4200, OUT = 450;
  let t1: ReturnType<typeof setTimeout>, t2: ReturnType<typeof setTimeout>;
  const leave = () => { if (leaving) return; leaving = true; clearTimeout(t1); t2 = setTimeout(onDone, OUT); };
  const key = (e: KeyboardEvent) => { if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') { e.preventDefault(); leave(); } };

  onMount(() => {
    t1 = setTimeout(leave, HOLD);
    dashboardData().then((d) => {
      const c = d.counts, s: typeof stats = [];
      if (d.total_tasks > 0) {
        s.push({ label: t(c.todo === 1 ? '{n} tarea por hacer' : '{n} tareas por hacer', { n: c.todo }) });
        if (c.overdue_recent) s.push({ label: t(c.overdue_recent === 1 ? '{n} vencida' : '{n} vencidas', { n: c.overdue_recent }), bad: true });
        if (c.exams_30d) s.push({ label: t(c.exams_30d === 1 ? '{n} examen en 30 días' : '{n} exámenes en 30 días', { n: c.exams_30d }) });
      }
      stats = s;
      lines = [`${t('sesión iniciada')}${first ? ' · ' + first : ''}`, t('datos locales cargados'), ...s.map((x) => x.label), t('listo')];
    }).catch(() => { lines = [`${t('sesión iniciada')}${first ? ' · ' + first : ''}`, t('listo')]; });
    return () => { clearTimeout(t1); clearTimeout(t2); };
  });
  // Cuadrícula: retraso según la distancia al centro (onda que se expande).
  const COLS = 13, ROWS = 7;
  const cells = Array.from({ length: COLS * ROWS }, (_, i) => { const x = i % COLS, y = Math.floor(i / COLS); return Math.hypot(x - (COLS - 1) / 2, (y - (ROWS - 1) / 2) * 1.6); });
</script>

<svelte:window onkeydown={key} />
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="w {kind}" class:leaving onclick={leave} role="dialog" tabindex="-1" aria-modal="true" aria-label={t('Bienvenida')} use:trapFocus>
  {#if kind === 'grid'}
    <div class="g" aria-hidden="true" style="--cols: {COLS}">{#each cells as d}<i style="animation-delay: {d * 110}ms"></i>{/each}</div>
  {/if}
  <div class="stage">
    {#if kind === 'type'}
      <span class="eyebrow">{greeting()}</span>
      <h1>{#each (first || 'AulaDesk').split('') as ch, i}<span style="animation-delay: {250 + i * 70}ms">{ch}</span>{/each}</h1>
      <span class="rule"></span>
      {#if stats.length}<p class="line">{stats.map((s) => s.label).join(' · ')}</p>{/if}
    {:else if kind === 'terminal'}
      <pre class="term" aria-label={lines.join('. ')}>{#each lines as l, i}<span class="ln" style="animation-delay: {300 + i * 420}ms"><b>&gt;</b> {l}</span>{/each}<span class="cur">▌</span></pre>
    {:else if kind === 'grid'}
      <div class="av">{name ? initials(name) : '✓'}</div>
      <h1 class="n">{greeting()}{first ? ', ' + first : ''}</h1>
      <div class="pills">{#each stats as s, i}<span class:bad={s.bad} style="animation-delay: {1100 + i * 130}ms">{s.label}</span>{/each}</div>
    {:else}
      <div class="mini">
        <span class="av s">{name ? initials(name) : '✓'}</span>
        <div><h1 class="n">{greeting()}{first ? ', ' + first : ''}</h1>
          {#if stats.length}<p class="line">{stats.map((s) => s.label).join(' · ')}</p>{/if}</div>
      </div>
    {/if}
  </div>
  <span class="skipmsg">{t('Pulsa para continuar')}</span>
  <span class="bar"><i style="animation-duration: {HOLD}ms"></i></span>
</div>

<style>
  .w { position: fixed; inset: 0; z-index: 200; display: grid; place-items: center; overflow: hidden; cursor: pointer; background: var(--bg); color: var(--text); animation: in .35s ease both; }
  .w.leaving { animation: out .45s ease forwards; }
  @keyframes in { from { opacity: 0; } } @keyframes out { to { opacity: 0; transform: scale(1.015); } }
  .stage { position: relative; z-index: 1; display: flex; flex-direction: column; align-items: center; text-align: center; gap: 12px; padding: 0 24px; }
  h1 { margin: 0; font-family: var(--font-display); font-weight: 650; }
  .skipmsg { position: absolute; bottom: 24px; font-size: 12px; color: var(--muted); }
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: var(--panel-3); } .bar i { display: block; height: 100%; background: var(--accent); transform-origin: left; transform: scaleX(0); animation: fill linear forwards; }
  @keyframes fill { to { transform: scaleX(1); } }
  .line { margin: 0; color: var(--muted); animation: up .6s ease 1.1s both; }
  @keyframes up { from { opacity: 0; transform: translateY(8px); } }

  /* Tipográfica */
  .eyebrow { font-size: 13px; letter-spacing: .32em; text-transform: uppercase; color: var(--muted); animation: up .6s ease .1s both; }
  .type h1 { font-size: clamp(56px, 12vw, 150px); letter-spacing: -.05em; line-height: 1; display: flex; }
  .type h1 span { display: inline-block; animation: pop .55s var(--ease-spring) both; } @keyframes pop { from { opacity: 0; transform: translateY(40%) scale(.6); } }
  .rule { width: 120px; height: 5px; background: var(--accent); border-radius: 3px; transform-origin: left; animation: grow .9s var(--ease) 1s both; } @keyframes grow { from { transform: scaleX(0); } }

  /* Terminal */
  .term { margin: 0; font: 500 clamp(15px, 2.2vw, 22px)/1.7 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; text-align: left; min-width: min(560px, 86vw); padding: 24px 28px; border: 1px solid var(--line-strong); border-radius: var(--radius-lg); background: var(--panel-solid); }
  .ln { display: block; opacity: 0; animation: lnin .01s linear forwards; } .ln b { color: var(--accent); }
  @keyframes lnin { to { opacity: 1; } }
  .cur { color: var(--accent); animation: blink 1s steps(1) infinite; } @keyframes blink { 50% { opacity: 0; } }

  /* Cuadrícula */
  .g { position: absolute; inset: 0; display: grid; grid-template-columns: repeat(var(--cols), 1fr); grid-auto-rows: 1fr; gap: 10px; padding: 6vh 6vw; }
  .g i { border-radius: 3px; background: var(--accent); opacity: .08; animation: cell 1.6s ease-out both; }
  @keyframes cell { 0% { opacity: 0; transform: scale(.4); } 25% { opacity: .9; transform: scale(1); } 100% { opacity: .1; } }
  .av { width: 92px; height: 92px; border-radius: var(--radius-lg); display: grid; place-items: center; background: var(--accent); color: var(--accent-fg); font: 800 34px/1 var(--font-display); animation: pop .6s var(--ease-spring) .5s both; }
  .av.s { width: 64px; height: 64px; font-size: 24px; animation: none; }
  .n { font-size: clamp(30px, 5vw, 60px); letter-spacing: -.03em; animation: up .7s ease .7s both; }
  .pills { display: flex; gap: 10px; flex-wrap: wrap; justify-content: center; } .pills span { padding: 6px 15px; border-radius: 99px; font-weight: 700; font-size: 13px; border: 1px solid var(--line-strong); background: var(--panel-solid); animation: up .5s ease both; } .pills .bad { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }

  /* Minimalista */
  .mini { display: flex; align-items: center; gap: 18px; padding: 22px 28px; border: 1px solid var(--line-strong); border-radius: var(--radius-lg); background: var(--panel-solid); box-shadow: var(--shadow-lg); animation: up .6s ease .1s both; text-align: left; }
  .mini .n { font-size: 24px; animation: none; } .mini .line { animation: none; margin-top: 2px; }
  @media (prefers-reduced-motion: reduce) { * { animation-duration: .01s !important; animation-delay: 0s !important; } }
  :global(:root[data-calm='1']) .w * { animation-duration: .01s !important; animation-delay: 0s !important; }
</style>
