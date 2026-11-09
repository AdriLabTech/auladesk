<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { driveStatus, driveSetClient, driveConnect, driveDisconnect, driveUploadPending, setSetting, type DriveStatus, type UploadOutcome } from './api';

  let st = $state<DriveStatus | null>(null);
  let error = $state(''); let notice = $state('');
  let clientId = $state(''); let clientSecret = $state('');
  let busy = $state<'save' | 'connect' | 'upload' | null>(null);
  let prog = $state<{ done: number; total: number; name: string } | null>(null);
  let outcome = $state<UploadOutcome[]>([]);
  let unlisten: UnlistenFn[] = [];
  const count = (s: string) => outcome.filter((o) => o.status === s).length;

  async function load() { try { st = await driveStatus(); if (st.client_id && !clientId) clientId = st.client_id; } catch (e) { error = String(e); } }
  onMount(async () => {
    load();
    unlisten.push(await listen('drive-status', load));
    unlisten.push(await listen<{ done: number; total: number; name: string }>('drive-progress', (e) => (prog = e.payload)));
  });
  onDestroy(() => unlisten.forEach((u) => u()));

  async function run(key: 'save' | 'connect' | 'upload', f: () => Promise<void>) { error = ''; notice = ''; busy = key; try { await f(); } catch (e) { error = String(e); } finally { busy = null; await load(); } }
  const save = () => run('save', async () => { await driveSetClient(clientId, clientSecret); clientSecret = ''; notice = 'Credenciales guardadas (el secreto está en el llavero del sistema).'; });
  const connect = () => run('connect', async () => { const email = await driveConnect(); notice = email ? `Conectado como ${email}.` : 'Drive conectado.'; });
  const disconnect = () => run('save', async () => { await driveDisconnect(false); notice = 'Drive desconectado. Los archivos ya subidos siguen en tu Drive.'; });
  const upload = () => run('upload', async () => { outcome = []; prog = null; outcome = await driveUploadPending(); prog = null; });
  const toggleBackup = (on: boolean) => run('save', async () => { await setSetting('drive_backup_submissions', on ? '1' : '0'); });
</script>

<section class="card">
  <h2>Google Drive</h2>
  <p class="muted">Guarda en tu Drive los recursos de tus clases y las entregas confirmadas, en la carpeta <b>AulaDesk</b> (con la misma estructura que la biblioteca). AulaDesk solo ve lo que él mismo sube (permiso <code>drive.file</code>); no lee el resto de tu Drive.</p>
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

  {#if st?.connected}
    <p class="state"><i class="pulse-dot" style="--t: var(--ok)"></i>Conectado{st.account ? ` como ${st.account}` : ''} · {st.uploaded} archivo(s) subido(s)</p>
    <div class="autorow"><div><b>Guardar en Drive cada entrega confirmada</b><small class="muted">Cuando Moodle confirme una entrega (a mano, desde la app o programada), se sube una copia a AulaDesk/…/Entregas.</small></div>
      <label class="row"><input type="checkbox" checked={st.backup_submissions} disabled={busy !== null} onchange={(e) => toggleBackup(e.currentTarget.checked)} /> Activado</label></div>
    <div class="row">
      <button class="btn primary" disabled={busy !== null} onclick={upload}>{busy === 'upload' ? 'Subiendo…' : 'Subir a Drive lo que falta'}</button>
      <button class="btn" disabled={busy !== null} onclick={disconnect}>Desconectar</button>
    </div>
    {#if busy === 'upload' && prog}<div class="prog" role="progressbar" aria-valuenow={prog.done} aria-valuemax={prog.total}><span style="width: {prog.total ? (prog.done / prog.total) * 100 : 0}%"></span></div><p class="muted small">{prog.done}/{prog.total} · {prog.name}</p>{/if}
    {#if outcome.length}
      <p>{count('uploaded')} subido(s), {count('skipped')} ya estaban, {count('error')} con error.</p>
      {#if count('error')}<ul class="err-list">{#each outcome.filter((o) => o.status === 'error').slice(0, 8) as o}<li>{o.name || `#${o.file_id}`}: {o.detail}</li>{/each}</ul>{/if}
    {/if}
  {:else}
    <p class="state"><i class="pulse-dot" style="--t: var(--warn)"></i>{st?.configured ? 'Credenciales guardadas; falta autorizar' : 'Sin configurar'}</p>
    <details open={!st?.configured}>
      <summary>Cómo conseguir el ID de cliente (una sola vez)</summary>
      <ol class="how">
        <li>En <code>console.cloud.google.com/apis/credentials</code> crea un proyecto y activa la <b>API de Google Drive</b>.</li>
        <li>Configura la pantalla de consentimiento (tipo «Externo», en pruebas) y añade tu correo como usuario de prueba.</li>
        <li>Crea una credencial «ID de cliente de OAuth» de tipo <b>Aplicación de escritorio</b> y pega aquí el ID y el secreto.</li>
      </ol>
    </details>
    <div class="fields">
      <label>ID de cliente<input bind:value={clientId} placeholder="…apps.googleusercontent.com" autocomplete="off" spellcheck="false" /></label>
      <label>Secreto del cliente<input type="password" bind:value={clientSecret} placeholder={st?.configured ? '•••••• (guardado)' : ''} autocomplete="off" /></label>
    </div>
    <div class="row">
      <button class="btn" disabled={busy !== null || !clientId || !clientSecret} onclick={save}>Guardar credenciales</button>
      <button class="btn primary" disabled={busy !== null || !st?.configured} onclick={connect}>{busy === 'connect' ? 'Esperando al navegador…' : 'Conectar con Google'}</button>
    </div>
    <p class="muted small">Se abrirá tu navegador para que autorices el acceso. El token se guarda en el llavero del sistema; AulaDesk nunca ve tu contraseña de Google.</p>
  {/if}
</section>

<style>
  section { max-width: 780px; margin-bottom: 16px; } .row { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 10px; } .small { font-size: 12.5px; }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; }
  .autorow { display: flex; justify-content: space-between; align-items: center; gap: 20px; padding: 12px 0; border-top: 1px solid var(--line); flex-wrap: wrap; } .autorow > div:first-child { display: flex; flex-direction: column; gap: 2px; max-width: 520px; } .autorow small { font-size: 12.5px; }
  .fields { display: grid; gap: 10px; margin: 12px 0; } .fields label { display: flex; flex-direction: column; gap: 4px; font-size: 12.5px; color: var(--muted); }
  details { margin: 8px 0; } summary { cursor: pointer; font-weight: 600; } .how { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; font-size: 13.5px; }
  .err-list { color: var(--bad); font-size: 13px; }
  .prog { height: 6px; border-radius: 99px; background: var(--panel-3); overflow: hidden; margin: 12px 0 4px; } .prog span { display: block; height: 100%; background: var(--accent); border-radius: 99px; transition: width .4s var(--ease); }
</style>
