<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { trapFocus } from '../lib/a11y';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open as pickFile } from '@tauri-apps/plugin-dialog';
  import { submissionHistory, submissionPrepare, submissionStart, submissionVerify, submissionCancel, submissionSubmit, scheduleList, scheduleCancel, fileReveal, fileOpen, type VersionRow, type ScheduleRow } from '../lib/api';
  import { fmtDate, fmtSize, versionState } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { openViewer } from '../lib/viewerstate.svelte';
  import ScheduleForm from '../lib/ScheduleForm.svelte';

  let { activityId = undefined, canPrepare = false, hasSubmission = false, onChange = () => {} }: { activityId?: number; canPrepare?: boolean; hasSubmission?: boolean; onChange?: () => void | Promise<void> } = $props();
  let rows = $state<VersionRow[]>([]);
  let schedules = $state<ScheduleRow[]>([]);
  let scheduling = $state<VersionRow | null>(null);
  const schedOf = (v: VersionRow) => schedules.find((s) => s.version_id === v.id && ['scheduled', 'running'].includes(s.state));
  let error = $state(''); let notice = $state('');
  let busy = $state<number | 'prep' | null>(null);
  let confirmStart = $state<VersionRow | null>(null);
  // Entrega desde la app
  let confirmSubmit = $state<VersionRow | null>(null); let agree = $state(false);
  type Run = { id: number; stage: string; text: string; error: string; done: boolean; ok: boolean };
  let subRun = $state<Run | null>(null);
  const STEPS: [string, string][] = $derived([['check', tt('Comprobar el archivo preparado')], ['open', tt('Abrir la entrega en Moodle')], ['upload', tt('Subir el archivo')], ['submit_save', tt('Guardar la entrega')], ['submit_final', tt('Enviar la tarea')], ['submit_confirm', tt('Confirmar el envío')], ['verify', tt('Verificar con Moodle (SHA-256)')]]);
  const stepState = (key: string) => {
    if (!subRun) return 'pending'; const cur = STEPS.findIndex((s) => s[0] === subRun!.stage), me = STEPS.findIndex((s) => s[0] === key);
    if (subRun.done) return 'done'; if (subRun.error && me === cur) return 'error'; return me < cur ? 'done' : me === cur ? 'current' : 'pending';
  };
  let unlisten: UnlistenFn | undefined;
  onMount(async () => { unlisten = await listen<{ stage: string; text: string }>('submit-progress', (e) => { if (subRun && !subRun.done) subRun = { ...subRun, stage: e.payload.stage === 'error' ? subRun.stage : e.payload.stage, text: e.payload.text }; }); });
  onDestroy(() => unlisten?.());
  async function doSubmit() {
    const v = confirmSubmit; if (!v || !agree) return;
    confirmSubmit = null; agree = false; subRun = { id: v.id, stage: 'check', text: 'Empezando…', error: '', done: false, ok: false };
    try { const r = await submissionSubmit(v.id, true, hasSubmission); subRun = { ...subRun!, done: true, ok: r.state === 'confirmed', text: r.state === 'confirmed' ? tt('Entrega confirmada por Moodle.') : tt('Entrega enviada; Moodle aún no muestra un archivo idéntico (puedes pulsar «Comprobar entrega» más tarde).') }; }
    catch (e) { subRun = { ...subRun!, error: String(e) }; }
    await load(); onChange();
  }

  async function load() { try { rows = await submissionHistory(activityId); schedules = await scheduleList(true); } catch (e) { error = String(e); } }
  onMount(load);

  async function run<T>(key: number | 'prep', f: () => Promise<T>, ok = '') {
    error = ''; notice = ''; busy = key;
    try { await f(); if (ok) notice = ok; await load(); onChange(); } catch (e) { error = String(e); } finally { busy = null; }
  }
  async function prepare() {
    if (activityId === undefined) return;
    const p = await pickFile({ multiple: false, directory: false });
    if (typeof p === 'string') await run('prep', () => submissionPrepare(activityId!, p), 'Copia preparada. Todavía NO se ha enviado nada a Moodle.');
  }
  const open = (v: VersionRow) => ['prepared', 'in_progress', 'pending_confirmation'].includes(v.state);
</script>

{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

{#if canPrepare && activityId !== undefined}
  <button class="btn primary sm" disabled={busy !== null} onclick={prepare}>{busy === 'prep' ? 'Preparando…' : 'Preparar entrega (elegir archivo)…'}</button>
{/if}

{#each rows as v (v.id)}
  {@const st = versionState[v.state] ?? { label: v.state, tone: 'muted' }}
  <article class="ver">
    <header>
      <strong>{activityId === undefined ? `${v.activity_name} · ` : ''}{tt('Versión')} {v.version}</strong>
      <span class="badge b-{st.tone}">{st.label}</span>
      {#if activityId === undefined}<span class="muted">{v.course_name}</span>{/if}
    </header>
    <p class="file"><b>{v.file_name}</b> <span class="muted">· {fmtSize(v.size_bytes)} · preparada {fmtDate(v.created_at)}</span></p>
    <p class="muted mono">SHA-256 {v.sha256}</p>
    <p class="muted small">{tt('Copia local:')} <code>{v.rel_path}</code></p>
    {#if v.sent_at}<p class="small">{tt('Moodle registra el envío:')} {fmtDate(v.sent_at)}{v.deadline_at ? ` · ${tt('fecha límite')} ${fmtDate(v.deadline_at)}` : ''}</p>{/if}
    {#if v.confirmation_evidence}<p class="evid small">{v.confirmation_evidence}</p>{/if}
    {#if v.error_detail}<p class="small err-t">{v.error_detail}</p>{/if}
    {#if schedOf(v)}{@const sc = schedOf(v)!}<p class="small sched"><Icon name="calendar" size={14} /> {tt('Entrega programada para')} <b>{fmtDate(sc.run_at)}</b>{sc.attempts ? ` · intento ${sc.attempts}` : ''}{sc.last_error ? ` · ${sc.last_error}` : ''}
      {#if sc.state === 'scheduled'}<button class="btn sm" disabled={busy !== null} onclick={() => run(v.id, () => scheduleCancel(sc.id), 'Programación cancelada.')}>{tt('Cancelar programación')}</button>{/if}</p>{/if}
    {#if v.grade}<p class="small">{tt('Calificación:')} <b>{v.grade}</b>{v.teacher_feedback ? ` · ${v.teacher_feedback}` : ''}</p>{/if}
    <div class="acts">
      <button class="btn sm" onclick={() => openViewer({ id: v.file_id, name: v.file_name })}>{tt('Ver copia')}</button>
      <button class="btn sm" onclick={() => fileOpen(v.file_id).catch((e) => (error = String(e)))}>{tt('Abrir con el sistema')}</button>
      <button class="btn sm" onclick={() => fileReveal(v.file_id).catch((e) => (error = String(e)))}>{tt('Mostrar carpeta')}</button>
      {#if open(v)}
        <button class="btn sm primary" disabled={busy !== null || subRun !== null} onclick={() => { agree = false; confirmSubmit = v; }}><Icon name="submissions" size={14} /> {hasSubmission ? 'Reemplazar entrega desde AulaDesk…' : 'Entregar desde AulaDesk…'}</button>
        {#if !schedOf(v)}<button class="btn sm" disabled={busy !== null || subRun !== null} onclick={() => (scheduling = v)}>{tt('Programar entrega…')}</button>{/if}
        <button class="btn sm" disabled={busy !== null} onclick={() => (confirmStart = v)}>{tt('Entregar a mano en Moodle…')}</button>
        <button class="btn sm" disabled={busy !== null} onclick={() => run(v.id, () => submissionVerify(v.id), 'Comprobación terminada: revisa el estado.')}>{busy === v.id ? 'Comprobando…' : 'Comprobar entrega'}</button>
        <button class="btn sm" disabled={busy !== null} onclick={() => run(v.id, () => submissionCancel(v.id), 'Versión cancelada (se conserva en el historial).')}>{tt('Cancelar')}</button>
      {/if}
    </div>
  </article>
{:else}
  <p class="muted">{activityId === undefined ? 'Aún no has preparado ninguna entrega.' : 'Sin versiones preparadas para esta tarea.'}</p>
{/each}

{#if scheduling}<ScheduleForm version={scheduling} onCancel={() => (scheduling = null)} onDone={async () => { scheduling = null; notice = 'Entrega programada. AulaDesk intentará entregar a esa hora (app abierta y sesión de Moodle iniciada).'; await load(); onChange(); }} />{/if}

{#if confirmSubmit}
  <div class="overlay" role="dialog" use:trapFocus aria-modal="true" aria-label={tt('Confirmar la entrega')}>
    <div class="modal card">
      <h2>{hasSubmission ? 'Modificar la entrega de' : 'Entregar'} «{confirmSubmit.activity_name}»</h2>
      <p class="muted small">{confirmSubmit.course_name}{confirmSubmit.deadline_at ? ` · plazo ${fmtDate(confirmSubmit.deadline_at)}` : ''}</p>
      <div class="sendbox">
        <div><span class="muted small">{tt('Archivo')}</span><b>{confirmSubmit.file_name}</b></div>
        <div><span class="muted small">{tt('Tamaño')}</span><b>{fmtSize(confirmSubmit.size_bytes)}</b></div>
        <div><span class="muted small">{tt('SHA-256')}</span><code title={confirmSubmit.sha256}>{confirmSubmit.sha256.slice(0, 24)}…</code></div>
      </div>
      {#if hasSubmission}<p class="msg err replace" role="alert"><b>{tt('Esta tarea ya tiene una entrega en Moodle.')}</b> {tt('Al continuar, AulaDesk')} <b>{tt('sustituirá los archivos ya entregados')}</b> {tt('por este (modificar entrega). Si no puede eliminar los anteriores, se detiene sin cambiar nada.')}</p>{/if}
      <p>{tt('AulaDesk subirá')} <b>{tt('exactamente este archivo')}</b> {tt('al formulario oficial de Moodle, guardará la entrega y, si la tarea lo exige, la enviará para calificar. Después comprobará que Moodle muestra un archivo con el mismo SHA-256.')}</p>
      <p class="muted small">{tt('Se detendrá sin enviar nada si la entrega ya tiene archivos, si Moodle muestra algo inesperado o si el archivo ha cambiado. En ese caso Moodle queda abierto para que termines a mano.')}</p>
      <label class="agree"><input type="checkbox" bind:checked={agree} /> {tt('Entiendo que esto')} <b>{hasSubmission ? 'reemplaza mi entrega en Moodle' : 'envía mi trabajo a Moodle'}</b> {tt('y lo verá el profesorado.')}</label>
      <div class="acts">
        <button class="btn primary" disabled={!agree} onclick={doSubmit}>{hasSubmission ? 'Reemplazar entrega ahora' : 'Entregar ahora'}</button>
        <button class="btn" onclick={() => (confirmSubmit = null)}>{tt('Cancelar')}</button>
      </div>
    </div>
  </div>
{/if}

{#if subRun}
  <div class="overlay" role="dialog" use:trapFocus aria-modal="true" aria-label={tt('Progreso de la entrega')}>
    <div class="modal card">
      <h2>{subRun.done ? (subRun.ok ? 'Entrega confirmada' : 'Entrega enviada') : subRun.error ? 'Entrega detenida' : 'Entregando…'}</h2>
      <ol class="steps">{#each STEPS as [k, label]}{@const st = stepState(k)}<li class={st}><i>{#if st === 'done'}<Icon name="check" size={12} stroke={3} />{:else if st === 'error'}<Icon name="x" size={12} stroke={3} />{/if}</i>{label}</li>{/each}</ol>
      {#if subRun.error}<p class="msg err" role="alert">{subRun.error}<br /><small>{tt('Moodle se ha abierto para que puedas terminar a mano. No se ha dado por confirmada ninguna entrega.')}</small></p>
      {:else if subRun.done}<p class="msg ok" role="status">{subRun.text}</p>
      {:else}<p class="muted small">{subRun.text}</p>{/if}
      <div class="acts"><button class="btn primary" disabled={!subRun.done && !subRun.error} onclick={() => (subRun = null)}>{subRun.done || subRun.error ? 'Cerrar' : 'Espera…'}</button></div>
    </div>
  </div>
{/if}

{#if confirmStart}
  <div class="overlay" role="dialog" use:trapFocus aria-modal="true" aria-label={tt('Confirmar apertura de la entrega')}>
    <div class="modal card">
      <h2>{tt('Abrir la entrega en Moodle')}</h2>
      <p>{tt('Se abrirá la página oficial de entrega de')} <b>«{confirmStart.activity_name}»</b>.</p>
      <p><b>{tt('AulaDesk no enviará nada.')}</b> {tt('Tendrás que adjuntar tú el archivo')} <b>{confirmStart.file_name}</b> {tt('(está en')} <code>{confirmStart.rel_path}</code>{tt(') y pulsar «Guardar cambios» y, si la tarea lo pide, «Enviar tarea».')}</p>
      <p class="muted">{tt('Después vuelve aquí y pulsa «Comprobar entrega»: solo se marcará como confirmada si Moodle muestra un archivo con el mismo SHA-256.')}</p>
      <div class="acts">
        <button class="btn primary" onclick={() => { const v = confirmStart!; confirmStart = null; run(v.id, () => submissionStart(v.id, true)); }}>{tt('Sí, abrir Moodle')}</button>
        <button class="btn" onclick={() => (confirmStart = null)}>No</button>
      </div>
    </div>
  </div>
{/if}


<style>
  .ver { background: var(--panel-2); border: 1px solid var(--line); border-radius: 14px; padding: 14px 16px; margin-top: 12px; animation: rise .4s var(--ease) backwards; transition: border-color .2s; } .ver:hover { border-color: var(--line-strong); } .ver p { margin: 4px 0; }
  header { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; } .small { font-size: 12.5px; } .mono { font-size: 11px; word-break: break-all; } .file { overflow-wrap: anywhere; }
  .evid { border-left: 3px solid var(--accent); padding-left: 10px; } .err-t { color: var(--bad); }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; }
  .replace { margin: 8px 0; } .sched { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .sendbox { display: grid; grid-template-columns: repeat(3, auto); gap: 6px 22px; justify-content: start; padding: 12px 14px; margin: 10px 0; background: var(--panel-2); border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: var(--radius); } .sendbox div { display: flex; flex-direction: column; min-width: 0; } .sendbox b { overflow-wrap: anywhere; }
  .agree { display: flex; gap: 10px; align-items: flex-start; margin: 12px 0 4px; font-size: 13.5px; }
  .steps { list-style: none; margin: 14px 0; padding: 0; display: flex; flex-direction: column; gap: 9px; } .steps li { display: flex; align-items: center; gap: 12px; color: var(--muted); font-size: 13.5px; }
  .steps i { width: 20px; height: 20px; border-radius: 50%; border: 2px solid var(--line-strong); display: grid; place-items: center; flex: none; font-style: normal; }
  .steps li.done { color: var(--text); } .steps li.done i { background: var(--ok); border-color: var(--ok); color: #fff; }
  .steps li.current { color: var(--text); font-weight: 650; } .steps li.current i { border-color: var(--accent); border-right-color: transparent; animation: spin .8s linear infinite; }
  .steps li.error { color: var(--bad); font-weight: 650; } .steps li.error i { background: var(--bad); border-color: var(--bad); color: #fff; }
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, .6); display: grid; place-items: center; z-index: 90; padding: 20px; animation: fade .2s backwards; } .modal { max-width: 580px; background: var(--panel-solid); box-shadow: var(--shadow-lg); animation: pop .3s var(--ease-spring) backwards; }
</style>