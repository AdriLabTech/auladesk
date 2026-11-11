<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  let { title, phase }: { title: string; phase: number } = $props();
</script>

<h1>{title}</h1>
<div class="card">
  <p>{tt('Sin datos todavía. Esta sección se implementa en la')} <strong>{tt('Fase')} {phase}</strong> {tt('y no muestra datos de ejemplo.')}</p>
  <p class="muted">{tt('Cuando la sincronización con Moodle esté activa, aquí aparecerá información real de tu cuenta.')}</p>
</div>
