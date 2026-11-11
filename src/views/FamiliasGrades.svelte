<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount } from 'svelte';
  import FamiliasHeader from '../lib/FamiliasHeader.svelte';
  import FamiliasEmpty from '../lib/FamiliasEmpty.svelte';
  import { fam, refreshFamilias, hasStudents } from '../lib/familias.svelte';
  import { familiasGrades, type GradeRow } from '../lib/api';

  let rows = $state<GradeRow[]>([]); let error = $state(''); let evalSel = $state('');
  async function load() { if (fam.activeId === null) { rows = []; return; } try { rows = await familiasGrades(fam.activeId); error = ''; } catch (e) { error = String(e); } }
  onMount(refreshFamilias);
  $effect(() => { fam.activeId; load(); });
  const evals = $derived([...new Set(rows.map((r) => r.evaluation))]);
  const shown = $derived(rows.filter((r) => !evalSel || r.evaluation === evalSel));
  const bySubject = $derived.by(() => { const m = new Map<string, GradeRow[]>(); for (const r of shown) (m.get(r.subject) ?? m.set(r.subject, []).get(r.subject)!).push(r); return [...m.entries()]; });
  const tone = (g: GradeRow) => (g.grade_num === null ? '' : g.grade_num < 5 ? 'bad' : g.grade_num >= 7 ? 'good' : '');
</script>

<div class="page">
<FamiliasHeader title="Calificaciones" />
{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if !hasStudents()}<FamiliasEmpty />
{:else}
  {#if evals.length > 1}<div class="chips"><button class="chip" class:on={!evalSel} onclick={() => (evalSel = '')}>{tt('Todas')}</button>{#each evals as e}<button class="chip" class:on={evalSel === e} onclick={() => (evalSel = e)}>{e}</button>{/each}</div>{/if}
  {#each bySubject as [subject, list] (subject)}
    <section class="card sub"><h2>{subject}</h2>
      {#each list as g (g.id)}
        <div class="li"><span><b>{g.evaluation}</b>{#if g.teacher}<small class="muted">{g.teacher}</small>{/if}{#if g.observations}<small class="obs">{g.observations}</small>{/if}</span><span class="gr {tone(g)}">{g.grade_text}</span></div>
      {/each}
    </section>
  {:else}<section class="card"><p>{tt('Sin calificaciones sincronizadas para este alumno.')}</p><p class="muted">{tt('Cuando el portal las muestre y se sincronicen, aparecerán aquí. No se muestra ningún dato que no venga del portal.')}</p></section>{/each}
{/if}
</div>


<style>
  .chips { margin: 4px 0 14px; } .sub { max-width: 780px; margin-bottom: 12px; animation: rise .4s var(--ease) backwards; }
  .li { display: flex; justify-content: space-between; gap: 12px; padding: 9px 0; border-top: 1px solid var(--line); } .li:first-of-type { border-top: 0; } .li span:first-child { display: flex; flex-direction: column; gap: 2px; }
  .obs { color: var(--text); white-space: pre-line; font-size: 12.5px; } .gr { font: 700 20px var(--font-display); align-self: center; } .gr.bad { color: var(--bad); } .gr.good { color: var(--ok); }
</style>