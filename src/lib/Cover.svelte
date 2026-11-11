<script lang="ts">
  import Icon from './Icon.svelte';
  import type { WelcomeKind } from './theme.svelte';
  let { kind }: { kind: Exclude<WelcomeKind, 'random'> } = $props();
</script>
<!-- Telón de arranque: tapa el Dashboard hasta que toma el relevo el saludo. Cada estilo de bienvenida trae el suyo. -->
<div class="cover {kind}" aria-hidden="true">
  {#if kind === 'type'}<span class="word">AulaDesk</span>
  {:else if kind === 'terminal'}<span class="t">&gt; AulaDesk<i>▌</i></span>
  {:else if kind === 'grid'}<span class="orb">{#each Array(8) as _, i}<i style="--i:{i}"></i>{/each}</span>
  {:else if kind === 'minimal'}<span class="line"><i></i></span>
  {:else}<span class="logo"><Icon name="cap" size={26} stroke={2} /></span>{/if}
</div>
<style>
  .cover { position: fixed; inset: 0; z-index: 150; display: grid; place-items: center; background: var(--bg); }
  .logo { width: 56px; height: 56px; border-radius: var(--radius); display: grid; place-items: center; color: var(--accent-fg); background: var(--accent); animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: .55; transform: scale(.94); } }
  .word { font: 650 clamp(36px, 7vw, 80px) var(--font-display); letter-spacing: -.04em; animation: pulse 1.4s ease-in-out infinite; }
  .t { font: 500 22px 'JetBrains Mono', ui-monospace, monospace; color: var(--text); } .t i { color: var(--accent); font-style: normal; animation: blink 1s steps(1) infinite; } @keyframes blink { 50% { opacity: 0; } }
  .orb { --s: 44px; --q: 10px; position: relative; width: calc(var(--s) + var(--q)); height: calc(var(--s) + var(--q)); }
  .orb i { position: absolute; width: var(--q); height: var(--q); background: var(--accent); border-radius: 2px; opacity: calc(1 - var(--i) * .1); animation: orbit 2.4s linear infinite; animation-delay: calc(var(--i) * -0.3s); }
  @keyframes orbit { 0% { transform: translate(0, 0); } 25% { transform: translate(var(--s), 0); } 50% { transform: translate(var(--s), var(--s)); } 75% { transform: translate(0, var(--s)); } 100% { transform: translate(0, 0); } }
  .line { width: 180px; height: 3px; background: var(--panel-3); border-radius: 2px; overflow: hidden; } .line i { display: block; width: 40%; height: 100%; background: var(--accent); animation: slide 1.1s ease-in-out infinite; } @keyframes slide { from { transform: translateX(-110%); } to { transform: translateX(260%); } }
  @media (prefers-reduced-motion: reduce) { * { animation: none !important; } }
</style>
