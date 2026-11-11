<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount } from 'svelte';
  import FamiliasHeader from '../lib/FamiliasHeader.svelte';
  import FamiliasEmpty from '../lib/FamiliasEmpty.svelte';
  import FamiliasAction from '../lib/FamiliasAction.svelte';
  import { fam, refreshFamilias, hasStudents } from '../lib/familias.svelte';
  import { familiasAttendance, familiasActions, type AttendanceRow, type ActionRow } from '../lib/api';
  import { fmtDate } from '../lib/format';

  let rows = $state<AttendanceRow[]>([]); let actions = $state<ActionRow[]>([]); let error = $state('');
  let filter = $state<'all' | 'unjustified'>('all'); let acting = $state<number | null>(null);
  async function load() {
    if (fam.activeId === null) { rows = []; actions = []; return; }
    try { [rows, actions] = await Promise.all([familiasAttendance(fam.activeId), familiasActions(fam.activeId)]); error = ''; } catch (e) { error = String(e); }
  }
  onMount(refreshFamilias);
  $effect(() => { fam.activeId; load(); });
  const shown = $derived(rows.filter((r) => filter === 'all' || (r.kind === 'falta' && !r.justified)));
  const dayText = (d: string) => new Date(d + 'T12:00:00').toLocaleDateString('es-ES', { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' });
  const pending = (id: number) => actions.some((a) => a.kind === 'justify_absence' && a.target_id === id && a.state === 'prepared');
  const stateTxt: Record<string, string> = { prepared: 'Preparada', running: 'Enviando', done: 'Enviada', failed: 'Fallida', cancelled: 'Cancelada', unsupported: 'No disponible' };
</script>

<div class="page">
<FamiliasHeader title="Faltas y retrasos" />
{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if !hasStudents()}<FamiliasEmpty />
{:else}
  <div class="chips"><button class="chip" class:on={filter === 'all'} onclick={() => (filter = 'all')}>{tt('Todas')}<i>{rows.length}</i></button><button class="chip" class:on={filter === 'unjustified'} onclick={() => (filter = 'unjustified')}>{tt('Sin justificar')}<i>{rows.filter((r) => r.kind === 'falta' && !r.justified).length}</i></button></div>
  <section class="card list">
    {#each shown as r (r.id)}
      <div class="li">
        <span class="d"><b>{dayText(r.day)}</b><small class="muted">{[r.slot, r.subject].filter(Boolean).join(' · ')}</small>{#if r.justification}<small>{r.justification}</small>{/if}</span>
        <span class="tags"><span class="badge" style="--t: {r.kind === 'retraso' ? 'var(--c-amber)' : 'var(--c-red)'}">{r.kind === 'retraso' ? 'Retraso' : r.kind === 'falta' ? 'Falta' : 'Otro'}</span>
          <span class="badge" style="--t: {r.justified ? 'var(--c-green)' : 'var(--c-muted)'}">{r.justified ? 'Justificada' : 'Sin justificar'}</span>
          {#if r.kind === 'falta' && !r.justified && !pending(r.id)}<button class="btn sm" onclick={() => (acting = r.id)}>{tt('Justificar…')}</button>{/if}</span>
      </div>
      {#if acting === r.id && fam.activeId !== null}<FamiliasAction studentId={fam.activeId} kind="justify_absence" attendanceId={r.id} title={`Justificar la falta del ${dayText(r.day)}`} onDone={() => { acting = null; load(); }} />{/if}
    {:else}<p class="muted">{tt('Sin faltas ni retrasos sincronizados para este alumno.')}</p>{/each}
  </section>
  {#if actions.length}
    <section class="card list"><h2>{tt('Acciones enviadas o preparadas')}</h2>
      {#each actions as a (a.id)}<div class="li"><span class="d"><b>{a.kind === 'justify_absence' ? 'Justificación de falta' : a.kind === 'request_tutoring' ? 'Petición de tutoría' : 'Mensaje'}</b><small class="muted">{fmtDate(a.created_at)}</small>{#if a.error}<small>{a.error}</small>{/if}</span><span class="badge" style="--t: var(--c-muted)">{stateTxt[a.state]}</span></div>{/each}
    </section>
  {/if}
{/if}
</div>


<style>
  .chips { margin: 4px 0 14px; } .list { max-width: 820px; margin-bottom: 14px; animation: rise .4s var(--ease) backwards; }
  .li { display: flex; justify-content: space-between; gap: 12px; align-items: center; flex-wrap: wrap; padding: 10px 0; border-top: 1px solid var(--line); } .li:first-of-type { border-top: 0; }
  .d { display: flex; flex-direction: column; gap: 2px; } .tags { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
</style>