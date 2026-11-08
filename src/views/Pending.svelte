<script lang="ts">
  let { title, phase }: { title: string; phase: number } = $props();
</script>

<h1>{title}</h1>
<div class="card">
  <p>Sin datos todavía. Esta sección se implementa en la <strong>Fase {phase}</strong> y no muestra datos de ejemplo.</p>
  <p class="muted">Cuando la sincronización con Moodle esté activa, aquí aparecerá información real de tu cuenta.</p>
</div>
