<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { familiasActionPrepare, familiasActionRun, familiasActionCancel, type ActionRow, type ActionParams } from './api';
  let { studentId, kind, attendanceId, title, onDone }: { studentId: number; kind: ActionRow['kind']; attendanceId?: number; title: string; onDone: () => void } = $props();
  let reason = $state(''); let subject = $state(''); let body = $state('');
  let step = $state<'edit' | 'confirm' | 'result'>('edit');
  let actionId = $state(0); let agree = $state(false); let error = $state(''); let result = $state<ActionRow | null>(null); let busy = $state(false);

  const params = $derived<ActionParams>(kind === 'send_message' ? { subject, body } : { attendance_id: attendanceId, reason });
  async function review() { error = ''; try { actionId = await familiasActionPrepare(studentId, kind, params); step = 'confirm'; } catch (e) { error = String(e); } }
  async function send() { error = ''; busy = true; try { result = await familiasActionRun(actionId, agree); step = 'result'; } catch (e) { error = String(e); } finally { busy = false; } }
  async function cancel() { try { if (step === 'confirm') await familiasActionCancel(actionId); } catch { /* ya no estaba preparada */ } onDone(); }
</script>

<div class="act card" role="group" aria-label={title}>
  <h3>{title}</h3>
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if step === 'edit'}
    {#if kind === 'send_message'}
      <label class="f">{tt('Asunto')}<input bind:value={subject} maxlength="200" /></label>
      <label class="f">{tt('Mensaje')}<textarea rows="4" bind:value={body} maxlength="5000"></textarea></label>
    {:else}
      <label class="f">{tt('Motivo')}<textarea rows="3" bind:value={reason} maxlength="1000" placeholder={tt('Escribe el motivo tal y como lo verá el centro')}></textarea></label>
    {/if}
    <div class="row"><button class="btn primary sm" onclick={review}>{tt('Revisar')}</button><button class="btn sm" onclick={onDone}>{tt('Cancelar')}</button></div>
  {:else if step === 'confirm'}
    <p><b>{tt('Esto es lo que se enviaría al portal, exactamente:')}</b></p>
    <blockquote>{#if kind === 'send_message'}<b>{subject}</b><br />{body}{:else}{reason}{/if}</blockquote>
    <p class="muted small">{tt('Es una acción sobre el expediente de un menor y no se puede deshacer desde AulaDesk. No se envía nada hasta que lo confirmes.')}</p>
    <label class="chk"><input type="checkbox" bind:checked={agree} /> {tt('Confirmo que quiero enviar esto al centro')}</label>
    <div class="row"><button class="btn primary sm" disabled={!agree || busy} onclick={send}>{tt('Enviar al portal')}</button><button class="btn sm" onclick={cancel}>{tt('Cancelar')}</button></div>
  {:else if result}
    <p class="msg {result.state === 'done' ? 'ok' : 'err'}" role="status">{result.state === 'done' ? 'Enviado al portal.' : (result.error ?? 'No se envió.')}</p>
    <button class="btn sm" onclick={onDone}>{tt('Cerrar')}</button>
  {/if}
</div>


<style>
  .act { margin: 10px 0; border-left: 4px solid var(--accent); } h3 { margin: 0 0 8px; } .row { display: flex; gap: 8px; margin-top: 10px; } .small { font-size: 12.5px; }
  blockquote { margin: 8px 0; padding: 10px 14px; border-left: 3px solid var(--line-strong); background: var(--panel-2); white-space: pre-line; overflow-wrap: anywhere; } .chk { display: flex; gap: 8px; align-items: center; }
</style>