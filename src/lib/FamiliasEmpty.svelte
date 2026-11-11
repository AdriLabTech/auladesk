<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { go } from './nav.svelte';
  import Icon from './Icon.svelte';
</script>
<section class="card empty">
  <span class="ico tone-blue"><Icon name="cap" size={26} /></span>
  <h2>{tt('Aún no hay alumnado de Familias')}</h2>
  <p class="muted">{tt('Conecta tu cuenta del portal de familias: AulaDesk lee el alumnado vinculado y sus datos con tu propia sesión, sin guardar tu contraseña.')}</p>
  <button class="btn primary" onclick={() => go('f_conn')}><Icon name="plug" size={15} /> {tt('Ir a Conexión')}</button>
</section>
