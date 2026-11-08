<script lang="ts">
  import { greeting, fmtAgo } from './format';
  import { account, initials } from './account.svelte';
  import { hero, fill } from './herocfg.svelte';
  import Icon from './Icon.svelte';
  import Counter from './Counter.svelte';

  type Props = { todo: number; overdue: number; topName?: string | null; topReason?: string | null; pct: number; lastSyncAt: number | null; ready?: boolean; preview?: boolean; onTasks?: () => void; onSync?: () => void };
  let { todo, overdue, topName = null, topReason = null, pct, lastSyncAt, ready = true, preview = false, onTasks = () => {}, onSync = () => {} }: Props = $props();

  const R = 52, C = 2 * Math.PI * R;
  const today = new Date().toLocaleDateString('es-ES', { weekday: 'long', day: 'numeric', month: 'long' });
  const vars = $derived({ nombre: account.name, tareas: todo, vencidas: overdue });
  const title = $derived(hero.title.trim() ? fill(hero.title, vars) : `${greeting()}.`);
  const customSub = $derived(hero.subtitle.trim() ? fill(hero.subtitle, vars) : '');
</script>

<section class="hero" class:neutral={hero.tone === 'panel'} class:noring={!hero.ring}>
  <div class="hero-txt">
    {#if hero.date}<span class="eyebrow">{today}</span>{/if}
    {#if hero.account && account.name}<span class="acct" title="Cuenta de Moodle con la sesión iniciada"><i>{initials(account.name)}</i>Sesión de <b>{account.name}</b>{#if account.userId}<small>ID {account.userId}</small>{/if}</span>{/if}
    <h1>{title}</h1>
    {#if customSub}<p class="lead">{customSub}</p>
    {:else if hero.summary}
      <p class="lead">
        {#if todo === 0}No tienes tareas pendientes. Buen momento para adelantar trabajo o repasar.
        {:else}
          Tienes <b>{todo}</b> {todo === 1 ? 'tarea por hacer' : 'tareas por hacer'}{#if overdue}, <b class="bad">{overdue}</b> vencida{overdue === 1 ? '' : 's'} recientemente{/if}.
          {#if topName}Lo primero: <b>{topName}</b>{#if topReason}, {topReason}{/if}.{/if}
        {/if}
      </p>
    {/if}
    {#if hero.actions}
      <div class="hero-actions">
        <button class="btn primary" onclick={onTasks} disabled={preview}><Icon name="tasks" size={16} /> Ver tareas</button>
        <button class="btn" onclick={onSync} disabled={preview}><Icon name="refresh" size={16} /> Sincronizar</button>
        <span class="synced"><i class="pulse-dot" style="--t: {lastSyncAt ? 'var(--ok)' : 'var(--warn)'}"></i>Sincronizado {fmtAgo(lastSyncAt)}</span>
      </div>
    {/if}
  </div>
  {#if hero.ring}
    <div class="ring" role="img" aria-label={`Progreso: ${pct}% de las tareas ya entregadas`}>
      <svg viewBox="0 0 120 120"><circle cx="60" cy="60" r={R} class="track" /><circle cx="60" cy="60" r={R} class="bar" stroke-dasharray={C} stroke-dashoffset={ready ? C * (1 - pct / 100) : C} /></svg>
      <div class="ring-in"><strong><Counter value={pct} suffix="%" /></strong><small>entregado</small></div>
    </div>
  {/if}
</section>

<style>
  .hero { display: flex; align-items: center; justify-content: space-between; gap: 28px; padding: 30px 34px; border-radius: var(--radius-lg); background: var(--accent); color: var(--accent-fg); position: relative; overflow: hidden; --fg: var(--accent-fg); --bgc: var(--accent); }
  .hero::after { content: ''; position: absolute; right: -70px; top: -70px; width: 260px; height: 260px; border-radius: 50%; border: 36px solid color-mix(in srgb, var(--fg) 8%, transparent); pointer-events: none; }
  .hero.neutral { background: var(--panel); color: var(--text); border: 1px solid var(--line); box-shadow: var(--shadow); --fg: var(--text); --bgc: var(--panel); }
  .hero.neutral .ring .bar { stroke: var(--accent); } .hero.neutral .eyebrow { color: var(--accent); opacity: 1; } .hero.neutral h1 { color: var(--text); }
  .hero.neutral .btn.primary { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); } .hero.neutral .btn { color: var(--text); border-color: var(--line-strong); }
  .hero-txt { min-width: 0; max-width: 680px; position: relative; z-index: 1; }
  .eyebrow { font-size: 12px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; opacity: .72; }
  .acct { display: inline-flex; align-items: center; gap: 9px; margin-left: 14px; padding: 3px 12px 3px 3px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 14%, transparent); font-size: 13px; vertical-align: middle; }
  .acct i { font-style: normal; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; background: var(--fg); color: var(--bgc); font: 800 11px/1 var(--font-display); }
  .acct b { font-weight: 700; } .acct small { opacity: .65; font-size: 11.5px; font-family: 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; }
  .hero h1 { font-size: 44px; margin: 6px 0 10px; letter-spacing: -.035em; line-height: 1.02; overflow-wrap: anywhere; }
  .lead { font-size: 15.5px; margin: 0 0 20px; line-height: 1.55; color: color-mix(in srgb, var(--fg) 78%, transparent); } .lead b { color: var(--fg); font-weight: 700; } .lead b.bad { text-decoration: underline wavy; text-underline-offset: 3px; }
  .hero-actions { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
  .hero .btn { background: transparent; border-color: color-mix(in srgb, var(--fg) 40%, transparent); color: var(--fg); } .hero .btn:hover:not(:disabled) { background: color-mix(in srgb, var(--fg) 10%, transparent); border-color: var(--fg); }
  .hero .btn.primary { background: var(--fg); color: var(--bgc); border-color: var(--fg); } .hero .btn.primary:hover:not(:disabled) { filter: none; opacity: .88; background: var(--fg); }
  .synced { display: inline-flex; gap: 9px; align-items: center; font-size: 12.5px; margin-left: 6px; opacity: .8; }
  .ring { position: relative; width: 170px; height: 170px; flex: none; z-index: 1; } .ring svg { width: 100%; height: 100%; transform: rotate(-90deg); }
  .ring circle { fill: none; stroke-width: 10; stroke-linecap: butt; } .track { stroke: color-mix(in srgb, var(--fg) 18%, transparent); } .ring .bar { stroke: var(--fg); transition: stroke-dashoffset 1.2s var(--ease) .2s; }
  .ring-in { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; }
  .ring-in strong { font-family: var(--font-display); font-size: 38px; letter-spacing: -.04em; line-height: 1; } .ring-in small { font-size: 12px; margin-top: 4px; opacity: .72; font-weight: 600; }
  @media (max-width: 760px) { .hero { flex-direction: column-reverse; align-items: flex-start; padding: 22px; } .ring { width: 130px; height: 130px; } .hero h1 { font-size: 34px; } }
</style>
