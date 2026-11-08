<script lang="ts">
  import { onMount } from 'svelte';
  import { dashboardData } from './api';
  import { account, initials } from './account.svelte';

  let { onDone }: { onDone: () => void } = $props();
  let leaving = $state(false);
  let stats = $state<{ label: string; tone?: string }[]>([]);
  const words = $derived((account.name ?? '').split(/\s+/).filter(Boolean));

  const HOLD = 4200, OUT = 650;
  let t1: ReturnType<typeof setTimeout>, t2: ReturnType<typeof setTimeout>;
  function leave() { if (leaving) return; leaving = true; clearTimeout(t1); t2 = setTimeout(onDone, OUT); }

  onMount(() => {
    t1 = setTimeout(leave, HOLD);
    // Cifras reales de tu BD local (si aún no hay datos, simplemente no se muestran)
    dashboardData().then((d) => {
      const c = d.counts, s: typeof stats = [];
      if (d.total_tasks > 0) {
        s.push({ label: `${c.todo} ${c.todo === 1 ? 'tarea por hacer' : 'tareas por hacer'}` });
        if (c.overdue_recent) s.push({ label: `${c.overdue_recent} ${c.overdue_recent === 1 ? 'vencida' : 'vencidas'}`, tone: 'bad' });
        if (c.exams_30d) s.push({ label: `${c.exams_30d} ${c.exams_30d === 1 ? 'examen' : 'exámenes'} en 30 días` });
      }
      stats = s;
    }).catch(() => {});
    return () => { clearTimeout(t1); clearTimeout(t2); };
  });
  const key = (e: KeyboardEvent) => { if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') { e.preventDefault(); leave(); } };
</script>

<svelte:window onkeydown={key} />
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" class:leaving onclick={leave} role="dialog" tabindex="-1" aria-modal="true" aria-label="Bienvenida">
  <i class="ring r1"></i><i class="ring r2"></i><i class="ring r3"></i>
  <div class="stage">
    <div class="badge-wrap">
      <svg class="draw" viewBox="0 0 120 120" aria-hidden="true"><circle cx="60" cy="60" r="54" /></svg>
      <div class="avatar">{account.name ? initials(account.name) : '✓'}</div>
      <svg class="tick" viewBox="0 0 32 32" aria-hidden="true"><circle cx="16" cy="16" r="16" class="tick-bg" /><path d="M9 16.5l4.8 4.8L23 11.5" /></svg>
    </div>
    <span class="eyebrow">Te damos la bienvenida</span>
    {#if words.length}
      <h1 class="name">{#each words as w, i}<span class="w"><span style="animation-delay: {1150 + i * 90}ms">{w}</span></span>{' '}{/each}</h1>
    {/if}
    <p class="sub">Sesión iniciada en Moodle · <span class="mono">educacionadistancia.juntadeandalucia.es</span></p>
    {#if stats.length}
      <div class="stats">{#each stats as s, i}<span class="pill" class:bad={s.tone === 'bad'} style="animation-delay: {1900 + i * 140}ms">{s.label}</span>{/each}</div>
    {/if}
  </div>
  <span class="skip">Pulsa para continuar</span>
  <span class="bar"><i style="animation-duration: {HOLD}ms"></i></span>
</div>

<style>
  .veil { position: fixed; inset: 0; z-index: 200; display: grid; place-items: center; overflow: hidden; cursor: pointer;
    background: var(--accent); color: var(--accent-fg); clip-path: circle(0% at 50% 46%); animation: reveal .8s var(--ease) forwards; }
  .veil.leaving { animation: leave .65s cubic-bezier(.7, 0, .84, 0) forwards; }
  @keyframes reveal { to { clip-path: circle(150% at 50% 46%); } }
  @keyframes leave { from { clip-path: inset(0 0 0 0); } to { clip-path: inset(0 0 100% 0); } }

  .veil::before { content: ''; position: absolute; inset: 0; pointer-events: none; opacity: .55;
    background-image: radial-gradient(color-mix(in srgb, var(--accent-fg) 22%, transparent) 1px, transparent 1.3px); background-size: 26px 26px;
    -webkit-mask-image: radial-gradient(circle at 50% 46%, #000 0, transparent 70%); mask-image: radial-gradient(circle at 50% 46%, #000 0, transparent 70%); }
  .ring { position: absolute; left: 50%; top: 46%; width: 220px; height: 220px; margin: -110px 0 0 -110px; border-radius: 50%; border: 2px solid color-mix(in srgb, var(--accent-fg) 30%, transparent); opacity: 0; animation: pulse 2.6s var(--ease) forwards; }
  .r1 { animation-delay: .45s; } .r2 { animation-delay: .75s; } .r3 { animation-delay: 1.05s; }
  @keyframes pulse { 0% { transform: scale(.5); opacity: .9; } 100% { transform: scale(7); opacity: 0; } }

  .stage { position: relative; z-index: 1; display: flex; flex-direction: column; align-items: center; text-align: center; gap: 10px; padding: 0 24px; max-width: 980px; }
  .badge-wrap { position: relative; width: 132px; height: 132px; margin-bottom: 14px; }
  .avatar { position: absolute; inset: 14px; border-radius: var(--radius-lg); display: grid; place-items: center; background: var(--accent-fg); color: var(--accent); font: 800 40px/1 var(--font-display); letter-spacing: .02em; transform: scale(.4); opacity: 0; animation: avatar .7s var(--ease-spring) .5s forwards; }
  @keyframes avatar { to { transform: scale(1); opacity: 1; } }
  .draw { position: absolute; inset: 0; width: 100%; height: 100%; transform: rotate(-90deg); }
  .draw circle { fill: none; stroke: var(--accent-fg); stroke-width: 3; stroke-linecap: round; stroke-dasharray: 340; stroke-dashoffset: 340; animation: stroke 1s var(--ease) .65s forwards; }
  @keyframes stroke { to { stroke-dashoffset: 0; } }
  .tick { position: absolute; right: -2px; bottom: -2px; width: 38px; height: 38px; transform: scale(0); animation: avatar .5s var(--ease-spring) 1.35s forwards; filter: drop-shadow(0 3px 6px rgba(0, 0, 0, .25)); }
  .tick-bg { fill: var(--ok); } .tick path { fill: none; stroke: #fff; stroke-width: 3.4; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 24; stroke-dashoffset: 24; animation: stroke .45s ease-out 1.6s forwards; }

  .eyebrow { font-size: 13px; font-weight: 700; letter-spacing: .32em; text-transform: uppercase; opacity: 0; animation: eyebrow 1s var(--ease) .95s forwards; }
  @keyframes eyebrow { from { opacity: 0; letter-spacing: .6em; } to { opacity: .75; letter-spacing: .22em; } }
  .name { font-family: var(--font-display); font-size: clamp(38px, 6.4vw, 84px); line-height: 1.02; letter-spacing: -.04em; margin: 4px 0 6px; font-weight: 650; }
  .w { display: inline-block; overflow: hidden; vertical-align: bottom; padding-bottom: .12em; }
  .w > span { display: inline-block; transform: translateY(110%); animation: up .85s var(--ease) forwards; }
  @keyframes up { to { transform: none; } }
  .sub { margin: 0; font-size: 14px; opacity: 0; animation: fadein .8s ease 1.7s forwards; } .mono { font-family: 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; font-size: 12.5px; }
  .stats { display: flex; gap: 10px; flex-wrap: wrap; justify-content: center; margin-top: 12px; }
  .pill { padding: 6px 15px; border-radius: 99px; font-weight: 700; font-size: 13px; background: color-mix(in srgb, var(--accent-fg) 14%, transparent); border: 1px solid color-mix(in srgb, var(--accent-fg) 30%, transparent); opacity: 0; transform: translateY(10px); animation: pillin .6s var(--ease) forwards; }
  .pill.bad { background: var(--accent-fg); color: var(--accent); border-color: var(--accent-fg); }
  @keyframes pillin { to { opacity: 1; transform: none; } } @keyframes fadein { to { opacity: .8; } }

  .skip { position: absolute; bottom: 26px; font-size: 12px; opacity: 0; animation: fadein 1s ease 2.4s forwards; letter-spacing: .04em; }
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 4px; background: color-mix(in srgb, var(--accent-fg) 14%, transparent); }
  .bar i { display: block; height: 100%; width: 100%; background: var(--accent-fg); transform-origin: left; transform: scaleX(0); animation: fill linear forwards; animation-delay: .8s; }
  @keyframes fill { to { transform: scaleX(1); } }
</style>
