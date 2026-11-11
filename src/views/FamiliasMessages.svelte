<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount } from 'svelte';
  import FamiliasHeader from '../lib/FamiliasHeader.svelte';
  import FamiliasEmpty from '../lib/FamiliasEmpty.svelte';
  import FamiliasAction from '../lib/FamiliasAction.svelte';
  import { fam, refreshFamilias, hasStudents } from '../lib/familias.svelte';
  import { familiasMessages, familiasMessageRead, type MessageRow } from '../lib/api';
  import { fmtDate } from '../lib/format';

  let rows = $state<MessageRow[]>([]); let error = $state(''); let kind = $state(''); let open = $state<number | null>(null); let composing = $state<null | 'send_message' | 'request_tutoring'>(null);
  async function load() { if (fam.activeId === null) { rows = []; return; } try { rows = await familiasMessages(fam.activeId); error = ''; } catch (e) { error = String(e); } }
  onMount(refreshFamilias);
  $effect(() => { fam.activeId; load(); });
  const kinds: [string, string][] = [['', 'Todo'], ['mensaje', 'Mensajes'], ['aviso', 'Avisos del centro'], ['observacion', 'Observaciones'], ['tutoria', 'Tutorías']];
  const shown = $derived(rows.filter((r) => !kind || r.kind === kind));
  const unread = (r: MessageRow) => !r.read_in_portal && !r.read_local_at;
  async function toggle(r: MessageRow) { open = open === r.id ? null : r.id; if (open === r.id && unread(r)) { try { await familiasMessageRead(r.id); await load(); } catch (e) { error = String(e); } } }
</script>

<div class="page">
<FamiliasHeader title="Comunicaciones" />
{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if !hasStudents()}<FamiliasEmpty />
{:else}
  <div class="bar"><div class="chips">{#each kinds as [k, l]}<button class="chip" class:on={kind === k} onclick={() => (kind = k)}>{l}</button>{/each}</div>
    <span class="row"><button class="btn sm" onclick={() => (composing = 'send_message')}>{tt('Escribir mensaje…')}</button><button class="btn sm" onclick={() => (composing = 'request_tutoring')}>{tt('Pedir tutoría…')}</button></span></div>
  {#if composing && fam.activeId !== null}<FamiliasAction studentId={fam.activeId} kind={composing} title={composing === 'send_message' ? 'Mensaje al centro' : 'Petición de tutoría'} onDone={() => (composing = null)} />{/if}
  <section class="list">
    {#each shown as r (r.id)}
      <button class="m card" class:unread={unread(r)} onclick={() => toggle(r)} aria-expanded={open === r.id}>
        <span class="h"><b>{r.subject}</b><small class="muted">{[kinds.find((x) => x[0] === r.kind)?.[1], r.sender, fmtDate(r.sent_at ?? r.first_seen_at)].filter(Boolean).join(' · ')}</small></span>
        {#if open === r.id}<span class="body">{r.body ?? 'Sin texto sincronizado.'}</span>{/if}
      </button>
    {:else}<section class="card"><p>{tt('Sin comunicaciones sincronizadas.')}</p></section>{/each}
  </section>
  <p class="muted small">{tt('«Leído» se marca solo en AulaDesk: no se cambia nada en el portal.')}</p>
{/if}
</div>


<style>
  .bar { display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin: 4px 0 14px; } .row { display: flex; gap: 8px; } .small { font-size: 12.5px; }
  .list { max-width: 820px; display: grid; gap: 8px; margin-bottom: 10px; }
  .m { display: flex; flex-direction: column; gap: 8px; text-align: left; color: var(--text); animation: rise .35s var(--ease) backwards; } .m.unread { border-left: 4px solid var(--accent); }
  .h { display: flex; flex-direction: column; gap: 2px; } .body { white-space: pre-line; overflow-wrap: anywhere; padding-top: 8px; border-top: 1px solid var(--line); }
</style>