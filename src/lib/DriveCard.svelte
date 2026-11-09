<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open as pickDir } from '@tauri-apps/plugin-dialog';
  import { folderBackupStatus, folderBackupSet, folderBackupSubmissionsSet, folderBackupRun, type FolderStatus, type CopyOutcome } from './api';
  import { driveStatus, driveSetClient, driveConnect, driveDisconnect, driveUploadPending, setSetting, type DriveStatus, type UploadOutcome } from './api';

  let st = $state<DriveStatus | null>(null);
  let error = $state(''); let notice = $state('');
  let clientId = $state(''); let clientSecret = $state('');
  let busy = $state<'save' | 'connect' | 'upload' | null>(null);
  let prog = $state<{ done: number; total: number; name: string } | null>(null);
  let outcome = $state<UploadOutcome[]>([]);
  let fs = $state<FolderStatus | null>(null);
  let copyRes = $state<CopyOutcome[]>([]);
  let copying = $state(false);
  const nCopy = (s: string) => copyRes.filter((o) => o.status === s).length;
  async function loadFolder() { try { fs = await folderBackupStatus(); } catch (e) { error = String(e); } }
  async function chooseFolder() {
    error = ''; notice = '';
    try { const p = await pickDir({ directory: true, multiple: false, title: 'Carpeta donde copiar tus archivos' }); if (typeof p === 'string') { await folderBackupSet(p); notice = 'Carpeta de copia guardada.'; } } catch (e) { error = String(e); }
    await loadFolder();
  }
  async function clearFolder() { try { await folderBackupSet(null); copyRes = []; } catch (e) { error = String(e); } await loadFolder(); }
  async function copyNow() { error = ''; copying = true; copyRes = []; try { copyRes = await folderBackupRun(); } catch (e) { error = String(e); } finally { copying = false; } }
  async function toggleFolderSubs(on: boolean) { try { await folderBackupSubmissionsSet(on); } catch (e) { error = String(e); } await loadFolder(); }
  let unlisten: UnlistenFn[] = [];
  const count = (s: string) => outcome.filter((o) => o.status === s).length;

  async function load() { try { st = await driveStatus(); if (st.client_id && !clientId) clientId = st.client_id; } catch (e) { error = String(e); } }
  onMount(async () => {
    load(); loadFolder();
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
  <h2>Copia en la nube</h2>
  <p class="muted">Guarda tus archivos de clase y tus entregas confirmadas fuera de este equipo, con la misma estructura que la biblioteca (carpeta <b>AulaDesk</b>). Elige la forma que te resulte más cómoda.</p>
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

  <h3>Opción 1 · Carpeta sincronizada <small class="tag">sin configurar nada</small></h3>
  <p class="muted small">Elige una carpeta que ya se sincronice con tu nube: la de <b>Google Drive para escritorio</b>, OneDrive, Dropbox, Nextcloud… AulaDesk solo copia ahí los archivos; no necesita cuentas ni permisos.</p>
  {#if fs?.path}
    <p class="state"><i class="pulse-dot" style="--t: {fs.available ? 'var(--ok)' : 'var(--warn)'}"></i>{fs.path}{fs.available ? '' : ' (no disponible ahora)'}</p>
    <div class="autorow"><div><b>Copiar cada entrega confirmada</b><small class="muted">Cuando Moodle confirme una entrega, se copia a AulaDesk/…/Entregas.</small></div>
      <label class="row"><input type="checkbox" checked={fs.submissions} onchange={(e) => toggleFolderSubs(e.currentTarget.checked)} /> Activado</label></div>
    <div class="row">
      <button class="btn primary" disabled={copying || !fs.available} onclick={copyNow}>{copying ? 'Copiando…' : 'Copiar mis archivos ahora'}</button>
      <button class="btn" onclick={chooseFolder}>Cambiar carpeta</button>
      <button class="btn" onclick={clearFolder}>Quitar</button>
    </div>
    {#if copyRes.length}<p>{nCopy('copied')} copiado(s), {nCopy('skipped')} ya estaban, {nCopy('error')} con error.</p>
      {#if nCopy('error')}<ul class="err-list">{#each copyRes.filter((o) => o.status === 'error').slice(0, 8) as o}<li>{o.name || `#${o.file_id}`}: {o.detail}</li>{/each}</ul>{/if}{/if}
  {:else}
    <div class="row"><button class="btn primary" onclick={chooseFolder}>Elegir carpeta…</button></div>
  {/if}

  <h3>Opción 2 · Conectar con Google Drive directamente</h3>
  <p class="muted small">AulaDesk solo ve lo que él mismo sube (permiso <code>drive.file</code>); no lee el resto de tu Drive.</p>

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
    {#if st?.bundled}
      <div class="row"><button class="btn primary" disabled={busy !== null} onclick={connect}>{busy === 'connect' ? 'Esperando al navegador…' : 'Conectar con Google'}</button></div>
      <p class="muted small">Se abrirá tu navegador para que autorices el acceso. El token se guarda en el llavero del sistema; AulaDesk nunca ve tu contraseña de Google.</p>
    {:else}
      <p class="muted small">Esta versión de AulaDesk no trae un acceso de Google propio, así que hay que crear tu ID de cliente (unos minutos, una sola vez). Si no quieres, usa la <b>Opción 1</b>.</p>
    {/if}
    <details open={!st?.bundled && !st?.configured}>
      <summary>{st?.bundled ? 'Avanzado: usar mi propio ID de cliente' : 'Cómo conseguir el ID de cliente (una sola vez)'}</summary>
      <ol class="how">
        <li>En <code>console.cloud.google.com/apis/credentials</code> crea un proyecto y activa la <b>API de Google Drive</b>.</li>
        <li>Pantalla de consentimiento: tipo «Externo». Añade tu correo como <b>usuario de prueba</b> (si no, Google responde «acceso bloqueado»).</li>
        <li>Crea un «ID de cliente de OAuth» de tipo <b>Aplicación de escritorio</b> y pega aquí el ID y el secreto.</li>
        <li>Con la app en modo «Pruebas», Google caduca la autorización a los 7 días: tendrás que volver a conectar.</li>
      </ol>
      <div class="fields">
        <label>ID de cliente<input bind:value={clientId} placeholder="…apps.googleusercontent.com" autocomplete="off" spellcheck="false" /></label>
        <label>Secreto del cliente<input type="password" bind:value={clientSecret} placeholder={st?.configured && !st?.bundled ? '•••••• (guardado)' : ''} autocomplete="off" /></label>
      </div>
      <div class="row">
        <button class="btn" disabled={busy !== null || !clientId || !clientSecret} onclick={save}>Guardar credenciales</button>
        {#if !st?.bundled}<button class="btn primary" disabled={busy !== null || !st?.configured} onclick={connect}>{busy === 'connect' ? 'Esperando al navegador…' : 'Conectar con Google'}</button>{/if}
      </div>
    </details>
  {/if}
</section>

<style>
  section { max-width: 780px; margin-bottom: 16px; } .row { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 10px; } .small { font-size: 12.5px; }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; }
  .autorow { display: flex; justify-content: space-between; align-items: center; gap: 20px; padding: 12px 0; border-top: 1px solid var(--line); flex-wrap: wrap; } .autorow > div:first-child { display: flex; flex-direction: column; gap: 2px; max-width: 520px; } .autorow small { font-size: 12.5px; }
  .fields { display: grid; gap: 10px; margin: 12px 0; } .fields label { display: flex; flex-direction: column; gap: 4px; font-size: 12.5px; color: var(--muted); }
  h3 { font-size: 14.5px; margin: 18px 0 4px; } .tag { font-weight: 600; font-size: 11px; color: var(--ok); margin-left: 6px; }
  details { margin: 8px 0; } summary { cursor: pointer; font-weight: 600; } .how { margin: 8px 0 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; font-size: 13.5px; }
  .err-list { color: var(--bad); font-size: 13px; }
  .prog { height: 6px; border-radius: 99px; background: var(--panel-3); overflow: hidden; margin: 12px 0 4px; } .prog span { display: block; height: 100%; background: var(--accent); border-radius: 99px; transition: width .4s var(--ease); }
</style>
