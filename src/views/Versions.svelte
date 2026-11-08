<script lang="ts">
  import { onMount } from 'svelte';
  import { open as pickFile } from '@tauri-apps/plugin-dialog';
  import { submissionHistory, submissionPrepare, submissionStart, submissionVerify, submissionCancel, fileReveal, fileOpen, type VersionRow } from '../lib/api';
  import { fmtDate, fmtSize, versionState } from '../lib/format';

  let { activityId = undefined, canPrepare = false, onChange = () => {} }: { activityId?: number; canPrepare?: boolean; onChange?: () => void } = $props();
  let rows = $state<VersionRow[]>([]);
  let error = $state(''); let notice = $state('');
  let busy = $state<number | 'prep' | null>(null);
  let confirmStart = $state<VersionRow | null>(null);

  async function load() { try { rows = await submissionHistory(activityId); } catch (e) { error = String(e); } }
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
      <strong>{activityId === undefined ? `${v.activity_name} · ` : ''}Versión {v.version}</strong>
      <span class="badge b-{st.tone}">{st.label}</span>
      {#if activityId === undefined}<span class="muted">{v.course_name}</span>{/if}
    </header>
    <p class="file"><b>{v.file_name}</b> <span class="muted">· {fmtSize(v.size_bytes)} · preparada {fmtDate(v.created_at)}</span></p>
    <p class="muted mono">SHA-256 {v.sha256}</p>
    <p class="muted small">Copia local: <code>{v.rel_path}</code></p>
    {#if v.sent_at}<p class="small">Moodle registra el envío: {fmtDate(v.sent_at)}{v.deadline_at ? ` · fecha límite ${fmtDate(v.deadline_at)}` : ''}</p>{/if}
    {#if v.confirmation_evidence}<p class="evid small">{v.confirmation_evidence}</p>{/if}
    {#if v.error_detail}<p class="small err-t">{v.error_detail}</p>{/if}
    {#if v.grade}<p class="small">Calificación: <b>{v.grade}</b>{v.teacher_feedback ? ` · ${v.teacher_feedback}` : ''}</p>{/if}
    <div class="acts">
      <button class="btn sm" onclick={() => fileOpen(v.file_id).catch((e) => (error = String(e)))}>Abrir copia</button>
      <button class="btn sm" onclick={() => fileReveal(v.file_id).catch((e) => (error = String(e)))}>Mostrar carpeta</button>
      {#if open(v)}
        <button class="btn sm primary" disabled={busy !== null} onclick={() => (confirmStart = v)}>Entregar en Moodle…</button>
        <button class="btn sm" disabled={busy !== null} onclick={() => run(v.id, () => submissionVerify(v.id), 'Comprobación terminada: revisa el estado.')}>{busy === v.id ? 'Comprobando…' : 'Comprobar entrega'}</button>
        <button class="btn sm" disabled={busy !== null} onclick={() => run(v.id, () => submissionCancel(v.id), 'Versión cancelada (se conserva en el historial).')}>Cancelar</button>
      {/if}
    </div>
  </article>
{:else}
  <p class="muted">{activityId === undefined ? 'Aún no has preparado ninguna entrega.' : 'Sin versiones preparadas para esta tarea.'}</p>
{/each}

{#if confirmStart}
  <div class="overlay" role="dialog" aria-modal="true" aria-label="Confirmar apertura de la entrega">
    <div class="modal card">
      <h2>Abrir la entrega en Moodle</h2>
      <p>Se abrirá la página oficial de entrega de <b>«{confirmStart.activity_name}»</b>.</p>
      <p><b>AulaDesk no enviará nada.</b> Tendrás que adjuntar tú el archivo <b>{confirmStart.file_name}</b> (está en <code>{confirmStart.rel_path}</code>) y pulsar «Guardar cambios» y, si la tarea lo pide, «Enviar tarea».</p>
      <p class="muted">Después vuelve aquí y pulsa «Comprobar entrega»: solo se marcará como confirmada si Moodle muestra un archivo con el mismo SHA-256.</p>
      <div class="acts">
        <button class="btn primary" onclick={() => { const v = confirmStart!; confirmStart = null; run(v.id, () => submissionStart(v.id, true)); }}>Sí, abrir Moodle</button>
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
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, .6); display: grid; place-items: center; z-index: 90; padding: 20px; animation: fade .2s backwards; } .modal { max-width: 580px; background: var(--panel-solid); box-shadow: var(--shadow-lg); animation: pop .3s var(--ease-spring) backwards; }
</style>
