<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount } from 'svelte';
  import FamiliasHeader from '../lib/FamiliasHeader.svelte';
  import FamiliasEmpty from '../lib/FamiliasEmpty.svelte';
  import Icon from '../lib/Icon.svelte';
  import { fam, refreshFamilias, hasStudents } from '../lib/familias.svelte';
  import { familiasOverview, type Overview } from '../lib/api';
  import { go } from '../lib/nav.svelte';
  import { fmtDate } from '../lib/format';

  let o = $state<Overview | null>(null); let error = $state('');
  async function load() { if (fam.activeId === null) { o = null; return; } try { o = await familiasOverview(fam.activeId); error = ''; } catch (e) { error = String(e); } }
  onMount(refreshFamilias);
  $effect(() => { fam.activeId; load(); });
</script>

<div class="page">
<FamiliasHeader title="Resumen del alumno" />
{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if !hasStudents()}<FamiliasEmpty />
{:else if o}
  <section class="kpis">
    <button class="kpi tone-violet" onclick={() => go('f_grades')}><span class="ico"><Icon name="star" /></span><span class="kv">{o.average !== null ? o.average.toFixed(1) : '—'}</span><span class="kl">{tt('Nota media')}</span><small>{o.grades} calificaciones · {o.subjects} asignaturas</small></button>
    <button class="kpi tone-red" onclick={() => go('f_attendance')}><span class="ico"><Icon name="clock" /></span><span class="kv">{o.absences}</span><span class="kl">{tt('Faltas')}</span><small>{o.unjustified} {tt('sin justificar')} · {o.delays} {tt('retrasos')}</small></button>
    <button class="kpi tone-blue" onclick={() => go('f_messages')}><span class="ico"><Icon name="msg" /></span><span class="kv">{o.unread}</span><span class="kl">{tt('Sin leer')}</span><small>{tt('comunicaciones')}</small></button>
  </section>
  <div class="cols">
    <section class="card"><h2>{tt('Últimas calificaciones')}</h2>
      {#each o.recent_grades as g (g.id)}<div class="li"><span><b>{g.subject}</b><small class="muted">{g.evaluation}</small></span><span class="gr">{g.grade_text}</span></div>
      {:else}<p class="muted">{tt('Sin calificaciones sincronizadas.')}</p>{/each}
    </section>
    <section class="card"><h2>{tt('Próximas tutorías y citas')}</h2>
      {#each o.next_events as [title, at]}<div class="li"><span><b>{title}</b></span><small class="muted">{fmtDate(at)}</small></div>
      {:else}<p class="muted">{tt('Nada programado.')}</p>{/each}
      <button class="btn sm" onclick={() => go('calendar')}>{tt('Ver calendario')}</button>
    </section>
  </div>
{/if}
</div>


<style>
  .kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 14px; margin-bottom: 18px; }
  .kpi { display: grid; gap: 2px; text-align: left; padding: 16px; background: var(--panel); border: 1px solid var(--line); border-radius: var(--radius-lg); color: var(--text); transition: border-color .15s, transform .2s var(--ease); } .kpi:hover { border-color: var(--accent-line); transform: translateY(-2px); }
  .kv { font: 700 30px/1.1 var(--font-display); } .kl { font-weight: 650; } .kpi small { color: var(--muted); } .ico { color: var(--accent); }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 14px; }
  .li { display: flex; justify-content: space-between; gap: 12px; padding: 9px 0; border-top: 1px solid var(--line); } .li:first-of-type { border-top: 0; } .li span { display: flex; flex-direction: column; }
  .gr { font: 700 18px var(--font-display); color: var(--accent); }
</style>