<script lang="ts">
  import { trapFocus } from './a11y';
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { busy } from './busy.svelte';
  import { t } from './i18n.svelte';
  import { nav } from './nav.svelte';
  import { newGame, step, turn, TICK, type Dir } from './snake';

  const SIZE = 11, CELL = 16, PX = SIZE * CELL;
  const ESCAPE_AFTER = 90; // s: si algo se cuelga, el usuario no se queda atrapado
  let dismissed = $state(false), secs = $state(0);
  let game = newGame(SIZE);
  let canvas = $state<HTMLCanvasElement>();
  let timer: ReturnType<typeof setInterval> | undefined, clock: ReturnType<typeof setInterval> | undefined, unlisten: (() => void) | undefined;
  let wasBusy = false;

  const active = $derived(busy.jobs.length > 0);
  // El saludo de bienvenida va primero: el loader aparece cuando termina (la sincronización sigue por detrás).
  const show = $derived(busy.blockSetting && !dismissed && active && !nav.welcome);
  const pct = $derived(busy.progress && busy.progress.total ? Math.round((busy.progress.done / busy.progress.total) * 100) : null);

  // Ciclo: empieza un trabajo → se reinicia; termina → se cierra al instante, aunque haya una partida en marcha.
  $effect(() => {
    if (active && !wasBusy) { wasBusy = true; dismissed = false; secs = 0; game = newGame(SIZE); clock = setInterval(() => (secs += 1), 1000); }
    if (!active && wasBusy) { wasBusy = false; clearInterval(clock); }
  });
  // Bloquea (inert) el resto de la app mientras el loader está visible, para que ni el teclado llegue a ella.
  $effect(() => { document.querySelector('.shell')?.toggleAttribute('inert', show); });
  // El bucle corre solo mientras el loader se ve (y el canvas existe).
  $effect(() => {
    if (!show || !canvas) return;
    draw(); timer = setInterval(() => { step(game); draw(); }, TICK);
    return () => clearInterval(timer);
  });

  const css = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim() || '#888';
  function draw() {
    const c = canvas?.getContext('2d'); if (!c) return;
    c.clearRect(0, 0, PX, PX);
    // Tablero: cuadrícula visible y borde marcado, para que se vean los límites del juego.
    c.lineWidth = 1; c.strokeStyle = css('--line');
    c.beginPath();
    for (let i = 1; i < SIZE; i++) { const p = i * CELL + 0.5; c.moveTo(p, 0); c.lineTo(p, PX); c.moveTo(0, p); c.lineTo(PX, p); }
    c.stroke();
    c.strokeStyle = css('--line-strong'); c.lineWidth = 2; c.strokeRect(1, 1, PX - 2, PX - 2);
    const q = CELL - 3;
    if (game.food) { c.fillStyle = css('--c-amber'); c.fillRect(game.food.x * CELL + 3, game.food.y * CELL + 3, q - 3, q - 3); }
    game.snake.forEach((p, i) => { c.globalAlpha = Math.max(.25, 1 - i * .1); c.fillStyle = css('--accent'); c.fillRect(p.x * CELL + 1, p.y * CELL + 1, q, q); });
    c.globalAlpha = 1;
  }
  const KEYS: Record<string, Dir> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right', w: 'up', s: 'down', a: 'left', d: 'right' };
  function onKey(e: KeyboardEvent) {
    if (!show) return;
    const d = KEYS[e.key] ?? KEYS[e.key.toLowerCase()];
    if (d) { turn(game, d); e.preventDefault(); }
  }
  // Deslizar o pulsar: pulsar gira hacia el lado del tablero donde se toca, respecto a la cabeza.
  let sx = 0, sy = 0;
  const toward = (dx: number, dy: number): Dir => (Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? 'right' : 'left') : dy > 0 ? 'down' : 'up');
  function down(e: PointerEvent) { sx = e.clientX; sy = e.clientY; }
  function up(e: PointerEvent) {
    const dx = e.clientX - sx, dy = e.clientY - sy;
    if (Math.max(Math.abs(dx), Math.abs(dy)) >= 24) return turn(game, toward(dx, dy));
    const r = canvas!.getBoundingClientRect(), h = game.snake[0];
    turn(game, toward(e.clientX - (r.left + (h.x + .5) * CELL), e.clientY - (r.top + (h.y + .5) * CELL)));
  }

  onMount(async () => { unlisten = await listen<{ done: number; total: number; current: string | null }>('sync-progress', (e) => (busy.progress = e.payload)); });
  onDestroy(() => { unlisten?.(); clearInterval(timer); clearInterval(clock); document.querySelector('.shell')?.removeAttribute('inert'); });
</script>

<svelte:window onkeydown={onKey} />
{#if show}
  <div class="ov" role="alertdialog" use:trapFocus aria-modal="true" aria-live="polite" aria-labelledby="ov-t" aria-describedby="ov-d">
    <div class="card">
      <canvas bind:this={canvas} width={PX} height={PX} onpointerdown={down} onpointerup={up} aria-label={t('Indicador de carga. Se puede dirigir con las flechas.')}></canvas>
      <h2 id="ov-t">{t('Sincronizando…')}</h2>
      <p id="ov-d" class="status">{busy.jobs.map((j) => t(j.label)).join(' · ')} · {secs} s</p>
      {#if pct !== null}<div class="bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}><span style="width: {pct}%"></span></div>
        <p class="status">{busy.progress?.done}/{busy.progress?.total}{busy.progress?.current ? ` · ${busy.progress.current}` : ''}</p>{/if}
      <p class="hint">{t('Mientras esperas, prueba las flechas del teclado')}</p>
      {#if secs >= ESCAPE_AFTER}<button class="btn ghost sm" onclick={() => (dismissed = true)} title={t('La sincronización sigue; podrás usar la app mientras termina')}>{t('Seguir en segundo plano')}</button>{/if}
    </div>
  </div>
{/if}

<style>
  .ov { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: color-mix(in srgb, var(--bg) 86%, transparent); backdrop-filter: blur(3px); animation: fade .2s ease; }
  .card { display: flex; flex-direction: column; align-items: center; gap: 10px; text-align: center; padding: 34px 40px; min-width: 340px; max-width: 92vw; background: var(--panel-solid); border: 1px solid var(--line-strong); border-radius: var(--radius-lg); box-shadow: var(--shadow-lg); }
  canvas { touch-action: none; max-width: 100%; margin-bottom: 4px; background: var(--bg); border-radius: 2px; }
  h2 { margin: 6px 0 0; font: 600 20px var(--font-display); letter-spacing: -.01em; }
  p { margin: 0; } .hint { font-size: 12px; color: var(--faint); margin-top: 4px; } .status { font-size: 12px; color: var(--muted); max-width: 320px; }
  .bar { width: 260px; height: 6px; background: var(--panel-3); border-radius: 99px; overflow: hidden; } .bar span { display: block; height: 100%; background: var(--accent); transition: width .3s var(--ease); }
  @keyframes fade { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .ov { animation: none; } }
</style>
