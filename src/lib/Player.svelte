<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { playerState, playerControl, type PlayerState } from './api';
  import { t } from './i18n.svelte';
  import { trapFocus } from './a11y';

  let { onClose }: { onClose: () => void } = $props();
  let s = $state<PlayerState | null>(null), err = $state(''), pos = $state(0), busy = $state(false);
  let poll: ReturnType<typeof setInterval> | undefined, tick: ReturnType<typeof setInterval> | undefined;

  const playing = $derived(s?.status === 'Playing');
  const fmt = (x: number) => `${Math.floor(x / 60)}:${String(Math.floor(x % 60)).padStart(2, '0')}`;
  const pct = $derived(s && s.length > 0 ? Math.min(100, (pos / s.length) * 100) : 0);

  async function refresh() { try { s = await playerState(); pos = s.position; err = ''; } catch (e) { err = String(e); } }
  async function act(a: 'play_pause' | 'next' | 'previous', ) {
    if (busy || !s) return; busy = true;
    if (a === 'play_pause') s = { ...s, status: s.status === 'Playing' ? 'Paused' : 'Playing' }; // respuesta inmediata; luego se reconcilia con el reproductor
    try { const r = await playerControl(a); if (a !== 'play_pause') { s = r; pos = r.position; } err = ''; } catch (e) { err = String(e); } finally { busy = false; }
    setTimeout(refresh, 700);
  }
  async function seek(e: MouseEvent | KeyboardEvent) {
    if (!s || s.length <= 0) return;
    const el = e.currentTarget as HTMLElement; let frac: number;
    if (e instanceof MouseEvent) { const r = el.getBoundingClientRect(); frac = (e.clientX - r.left) / r.width; }
    else if (e.key === 'ArrowRight') frac = (pos + 5) / s.length; else if (e.key === 'ArrowLeft') frac = (pos - 5) / s.length; else return;
    e.preventDefault(); frac = Math.min(1, Math.max(0, frac));
    pos = frac * s.length;
    try { s = await playerControl('seek', pos); } catch (er) { err = String(er); }
  }
  function key(e: KeyboardEvent) { if (e.key === 'Escape') { e.stopPropagation(); onClose(); } }

  onMount(() => {
    refresh();
    poll = setInterval(refresh, 1500);                      // reconcilia con lo que hace el reproductor (cambios de canción, pausas desde fuera)
    tick = setInterval(() => { if (playing && s && s.length > 0) pos = Math.min(s.length, pos + 0.25); }, 250); // avanza la barra entre consultas
  });
  onDestroy(() => { clearInterval(poll); clearInterval(tick); });
</script>

<svelte:window onkeydown={key} />
<div class="pl" role="dialog" aria-label={t('Reproductor de música')} use:trapFocus>
  {#if s?.available}
    <div class="meta" aria-live="polite">
      <b class="ti" title={s.title}>{s.title || t('Sin canción')}</b>
      <span class="ar">{[s.artist, s.album].filter(Boolean).join(' · ') || ' '}</span>
      <small class="src">{t(playing ? 'Sonando en {p}' : 'En pausa en {p}', { p: s.player })}</small>
    </div>
    {#if s.length > 0}
      <div class="bar" role="slider" tabindex="0" aria-label={t('Posición')} aria-valuemin={0} aria-valuemax={Math.round(s.length)} aria-valuenow={Math.round(pos)} aria-valuetext={`${fmt(pos)} / ${fmt(s.length)}`} onclick={seek} onkeydown={seek}><i style="width: {pct}%"></i></div>
      <div class="tm"><span>{fmt(pos)}</span><span>{fmt(s.length)}</span></div>
    {/if}
    <div class="ctl">
      <button class="b" onclick={() => act('previous')} disabled={busy} aria-label={t('Canción anterior')} title={t('Canción anterior')}><svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M6 5h2v14H6zM20 5v14L9 12z" /></svg></button>
      <button class="b main" onclick={() => act('play_pause')} disabled={busy} aria-label={playing ? t('Pausar') : t('Reproducir')} title={playing ? t('Pausar') : t('Reproducir')}>
        {#if playing}<svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path d="M6 4h4v16H6zM14 4h4v16h-4z" /></svg>{:else}<svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"><path d="M7 4v16l13-8z" /></svg>{/if}
      </button>
      <button class="b" onclick={() => act('next')} disabled={busy} aria-label={t('Canción siguiente')} title={t('Canción siguiente')}><svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M16 5h2v14h-2zM4 5v14l11-7z" /></svg></button>
    </div>
  {:else if s}
    <p class="none"><b>{t('Sin reproductor')}</b><br /><span class="muted">{t(s.reason || 'No hay ningún reproductor abierto.')} {t('Abre Spotify, VLC o cualquier reproductor y aparecerá aquí.')}</span></p>
  {:else}<p class="none muted">{t('Cargando…')}</p>{/if}
  {#if err}<p class="er" role="alert">{err}</p>{/if}
</div>

<style>
  .pl { display: flex; flex-direction: column; gap: 8px; padding: 12px; background: var(--panel-solid); border: 1px solid var(--line-strong); border-radius: var(--radius-lg); box-shadow: var(--shadow-lg); }
  .meta { display: flex; flex-direction: column; min-width: 0; line-height: 1.3; }
  .ti, .ar { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .ti { font-size: 14px; } .ar { font-size: 12.5px; color: var(--muted); } .src { font-size: 11px; color: var(--muted); margin-top: 2px; }
  .bar { height: 8px; border-radius: 99px; background: var(--panel-3); cursor: pointer; position: relative; overflow: hidden; } .bar i { display: block; height: 100%; background: var(--accent); }
  .bar:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .tm { display: flex; justify-content: space-between; font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; margin-top: -4px; }
  .ctl { display: flex; align-items: center; justify-content: center; gap: 10px; }
  .b { width: 36px; height: 36px; display: grid; place-items: center; border-radius: var(--radius); background: var(--panel-2); border: 1px solid var(--line-strong); color: var(--text); cursor: pointer; transition: background .15s, border-color .15s; }
  .b:hover:not(:disabled) { background: var(--panel-3); border-color: var(--accent-line); } .b:disabled { opacity: .55; cursor: default; }
  .b svg { fill: currentColor; } .b.main { width: 46px; height: 46px; background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
  .none { margin: 0; font-size: 12.5px; line-height: 1.45; } .er { margin: 0; font-size: 12px; color: var(--bad); }
</style>
