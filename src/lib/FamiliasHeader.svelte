<script lang="ts">
  import { fmtDate } from './format';
  import { t as tt } from './i18n.svelte';
  import { fam, pickStudent, activeStudent } from './familias.svelte';
  import { go } from './nav.svelte';
  import Icon from './Icon.svelte';
  let { title }: { title: string } = $props();
  const st = $derived(activeStudent());
</script>

<header class="fh">
  <div class="tt"><span class="plat">{tt('Familias')}</span><h1>{title}</h1></div>
  {#if fam.status && fam.status.students.length > 0}
    <label class="ctx" title={tt('Alumno activo: todo lo que ves en esta pantalla es solo de este alumno')}>
      <Icon name="cap" size={15} />
      {#if fam.status.students.length > 1}
        <select value={String(fam.activeId)} onchange={(e) => pickStudent(Number(e.currentTarget.value))} aria-label={tt('Alumno activo')}>
          {#each fam.status.students as s (s.id)}<option value={String(s.id)}>{s.display_name}</option>{/each}
        </select>
      {:else}<b>{st?.display_name}</b>{/if}
      {#if st?.course_group || st?.center}<small>{[st?.course_group, st?.center].filter(Boolean).join(' · ')}</small>{/if}
    </label>
  {/if}
</header>
{#if fam.status && fam.status.session !== 'active'}
  <p class="stale" role="status"><Icon name="alert" size={15} /> {tt('Sin sesión en el portal: lo que ves son los últimos datos guardados')}{fam.status.last_sync_at ? ` (${tt('sincronizados el')} ${fmtDate(fam.status.last_sync_at)})` : ''}, {tt('no la situación actual.')} <button class="btn sm" onclick={() => go('f_conn')}>{tt('Conexión')}</button></p>
{/if}


<style>
  .fh { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; flex-wrap: wrap; margin-bottom: 14px; }
  .tt { display: flex; flex-direction: column; gap: 2px; } .tt h1 { margin: 0; }
  .plat { font-size: 11px; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; color: var(--accent); }
  .ctx { display: inline-flex; align-items: center; gap: 10px; padding: 7px 12px; background: var(--panel-2); border: 1px solid var(--line-strong); border-left: 4px solid var(--accent); border-radius: var(--radius); }
  .ctx select { background: transparent; border: 0; font-weight: 650; color: var(--text); } .ctx small { color: var(--muted); }
  .stale { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 10px 14px; margin: 0 0 16px; border: 1px solid color-mix(in srgb, var(--warn) 50%, transparent); border-left: 4px solid var(--warn); border-radius: var(--radius); background: color-mix(in srgb, var(--warn) 9%, var(--panel)); font-size: 13px; }
</style>