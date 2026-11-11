<script lang="ts">
  import { look, still } from './theme.svelte';
  import { audio } from './audio.svelte';
  import Player from './Player.svelte';
  import { t } from './i18n.svelte';
  let open = $state(false);
  let { place }: { place: 'side' | 'top' } = $props();
  // Elementos decorativos: no reaccionan a nada, no reciben el ratón, no se anuncian y se apagan con el modo tranquilo.
  const bars = Array.from({ length: 16 }, (_, i) => ({ d: 0.7 + ((i * 37) % 11) / 10, o: -(((i * 53) % 17) / 10), h: 35 + ((i * 29) % 60) }));
  const on = $derived(!still());
  const avg = $derived(audio.levels.reduce((a, b) => a + b, 0) / audio.levels.length);
</script>
{#if on && place === 'side' && look.fx === 'eq'}
  <div class="host">
    {#if open}<div class="pop"><Player onClose={() => (open = false)} /></div>{/if}
    <button class="eq" onclick={() => (open = !open)} aria-expanded={open} aria-haspopup="dialog" aria-label={t('Reproductor de música')} title={t('Pulsa para controlar la música')}>
      {#each bars as b, i}<i class:live={audio.live} style="--d:{b.d}s;--o:{b.o}s;--h:{b.h}%;--lv:{Math.max(0.05, audio.levels[i] ?? 0)}"></i>{/each}
    </button>
  </div>
{:else if on && place === 'top' && look.fx === 'wave'}
  <svg class="wave" viewBox="0 0 400 20" preserveAspectRatio="none" aria-hidden="true"><g class="wv" class:live={audio.live} style="--amp:{0.15 + avg * 1.6}"><path d="M0 10 Q 25 -2 50 10 T 100 10 T 150 10 T 200 10 T 250 10 T 300 10 T 350 10 T 400 10 T 450 10 T 500 10 T 550 10 T 600 10" /></g></svg>
{/if}
<style>
  .host { position: relative; }
  .pop { position: absolute; left: 0; right: 0; bottom: calc(100% + 8px); z-index: 30; }
  .eq { display: flex; align-items: flex-end; gap: 3px; height: 30px; width: 100%; padding: 0 6px; opacity: .75; background: none; border: 0; border-radius: var(--radius); cursor: pointer; transition: opacity .15s, background .15s; } .eq:hover, .eq[aria-expanded='true'] { opacity: 1; background: var(--panel-2); }
  .eq i { flex: 1; min-width: 2px; height: var(--h); background: var(--accent); border-radius: 1px; transform-origin: bottom; animation: eq var(--d) ease-in-out var(--o) infinite alternate; }
  .eq i.live { height: 100%; animation: none; transform: scaleY(var(--lv)); transition: transform .06s linear; opacity: calc(.5 + var(--lv) * .5); }
  .wv.live { transform-box: fill-box; }
  .wave .wv.live path { transform: scaleY(var(--amp)); transform-origin: center; transform-box: fill-box; transition: transform .08s linear; }
  @keyframes eq { from { transform: scaleY(.15); } to { transform: scaleY(1); } }
  .wave { position: absolute; left: 0; right: 0; bottom: -1px; width: 100%; height: 14px; pointer-events: none; opacity: .55; overflow: hidden; }
  .wv { animation: wave 7s linear infinite; } .wave path { fill: none; stroke: var(--accent); stroke-width: 1.4; vector-effect: non-scaling-stroke; }
  @keyframes wave { to { transform: translateX(-200px); } }
</style>
