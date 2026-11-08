<script lang="ts">
  import Versions from './Versions.svelte';
</script>

<h1>Entregas</h1>
<p class="muted">Historial de todo lo que has preparado y entregado. Cada versión conserva su copia y su SHA-256, y solo figura como «confirmada» si Moodle muestra un archivo idéntico. Prepara una entrega desde «Tareas y actividades».</p>
<div class="wrap"><Versions /></div>

<style>
  .wrap { max-width: 900px; margin-top: 14px; }
</style>
