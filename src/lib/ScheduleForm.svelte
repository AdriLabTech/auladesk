<script lang="ts">
  import { scheduleCreate, type VersionRow } from './api';
  import { fmtDate, fmtSize } from './format';

  let { version, onDone, onCancel }: { version: VersionRow; onDone: () => void; onCancel: () => void } = $props();
  // Valor por defecto: 1 h antes del cierre (si hay plazo y queda tiempo) o dentro de 1 h.
  const toLocal = (t: number) => { const d = new Date(t * 1000); d.setMinutes(d.getMinutes() - d.getTimezoneOffset()); return d.toISOString().slice(0, 16); };
  function defaultWhen() { const now = Math.floor(Date.now() / 1000); const d = version.deadline_at; return toLocal(d && d - 3600 > now + 120 ? d - 3600 : now + 3600); }
  let when = $state(defaultWhen());
  let agree = $state(false); let error = $state(''); let busy = $state(false);
  const runAt = $derived(Math.floor(new Date(when).getTime() / 1000));
  const problem = $derived(!Number.isFinite(runAt) ? 'Elige fecha y hora.' : runAt < Math.floor(Date.now() / 1000) + 60 ? 'La hora debe estar en el futuro.' : version.deadline_at && runAt > version.deadline_at ? 'Es posterior al cierre de la tarea.' : '');

  async function save() {
    if (problem || !agree) return;
    busy = true; error = '';
    try { await scheduleCreate(version.id, runAt, true); onDone(); } catch (e) { error = String(e); } finally { busy = false; }
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" aria-label="Programar la entrega">
  <div class="modal card">
    <h2>Programar «{version.activity_name}»</h2>
    <p class="muted small">{version.course_name}{version.deadline_at ? ` · plazo ${fmtDate(version.deadline_at)}` : ' · sin fecha límite conocida'}</p>
    <div class="sendbox">
      <div><span class="muted small">Archivo</span><b>{version.file_name}</b></div>
      <div><span class="muted small">Tamaño</span><b>{fmtSize(version.size_bytes)}</b></div>
      <div><span class="muted small">SHA-256</span><code title={version.sha256}>{version.sha256.slice(0, 24)}…</code></div>
    </div>
    <label class="when">Intentar entregar el <input type="datetime-local" bind:value={when} /></label>
    {#if problem}<p class="msg err" role="alert">{problem}</p>{/if}
    <p class="muted small">A esa hora AulaDesk subirá <b>exactamente este archivo</b> y comprobará con Moodle que el SHA-256 coincide. Necesita <b>la app abierta (o en la bandeja) y la sesión de Moodle iniciada</b>: la sesión caduca tras 8 h sin uso. Si no hay sesión, reintenta cada 10 min (máx. 3 intentos) hasta el cierre y te avisa. No entrega si el archivo cambia, si la entrega ya tiene archivos o si el plazo ha cerrado.</p>
    <label class="agree"><input type="checkbox" bind:checked={agree} /> Entiendo que a esa hora AulaDesk <b>enviará mi trabajo a Moodle sin pedirme nada más</b>.</label>
    {#if error}<p class="msg err" role="alert">{error}</p>{/if}
    <div class="acts">
      <button class="btn primary" disabled={!agree || !!problem || busy} onclick={save}>{busy ? 'Programando…' : 'Programar entrega'}</button>
      <button class="btn" onclick={onCancel}>Cancelar</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, .6); display: grid; place-items: center; z-index: 95; padding: 20px; animation: fade .2s backwards; }
  .modal { max-width: 580px; background: var(--panel-solid); box-shadow: var(--shadow-lg); animation: pop .3s var(--ease-spring) backwards; }
  .small { font-size: 12.5px; }
  .sendbox { display: grid; grid-template-columns: repeat(3, auto); gap: 6px 22px; justify-content: start; padding: 12px 14px; margin: 10px 0; background: var(--panel-2); border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: var(--radius); } .sendbox div { display: flex; flex-direction: column; min-width: 0; } .sendbox b { overflow-wrap: anywhere; }
  .when { display: flex; align-items: center; gap: 12px; margin: 12px 0; font-weight: 600; }
  .agree { display: flex; gap: 10px; align-items: flex-start; margin: 12px 0 4px; font-size: 13.5px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 12px; }
</style>
