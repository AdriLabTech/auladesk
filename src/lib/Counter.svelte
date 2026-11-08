<script lang="ts">
  import { onMount } from 'svelte';
  /** Número que sube hasta su valor (ease-out). Con «reducir movimiento» muestra el valor final directamente. */
  let { value, duration = 900, suffix = '' }: { value: number; duration?: number; suffix?: string } = $props();
  let shown = $state(0);
  let raf = 0;
  let from = 0;

  function run(to: number) {
    cancelAnimationFrame(raf);
    if (matchMedia('(prefers-reduced-motion: reduce)').matches || document.documentElement.dataset.calm === '1') { shown = to; from = to; return; }
    const start = performance.now(), a = from;
    const tick = (t: number) => {
      const p = Math.min(1, (t - start) / duration), e = 1 - Math.pow(1 - p, 4);
      shown = Math.round(a + (to - a) * e);
      if (p < 1) raf = requestAnimationFrame(tick); else from = to;
    };
    raf = requestAnimationFrame(tick);
  }
  onMount(() => () => cancelAnimationFrame(raf));
  $effect(() => { run(value); });
</script>

<span class="num">{shown}{suffix}</span>

<style> .num { font-variant-numeric: tabular-nums; } </style>
