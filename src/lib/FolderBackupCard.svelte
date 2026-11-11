<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { onMount } from 'svelte';
  import { open as pickDir } from '@tauri-apps/plugin-dialog';
  import { folderBackupStatus, folderBackupSet, folderBackupSubmissionsSet, folderBackupRun, type FolderStatus, type CopyOutcome } from './api';

  let fs = $state<FolderStatus | null>(null);
  let error = $state(''); let notice = $state('');
  let res = $state<CopyOutcome[]>([]);
  let copying = $state(false);
  const n = (s: string) => res.filter((o) => o.status === s).length;

  async function load() { try { fs = await folderBackupStatus(); } catch (e) { error = String(e); } }
  onMount(load);
  async function choose() {
    error = ''; notice = '';
    try { const p = await pickDir({ directory: true, multiple: false, title: tt('Carpeta donde copiar tus archivos') }); if (typeof p === 'string') { await folderBackupSet(p); notice = tt('Carpeta de copia guardada.'); } } catch (e) { error = String(e); }
    await load();
  }
  async function clear() { try { await folderBackupSet(null); res = []; } catch (e) { error = String(e); } await load(); }
  async function copyNow() { error = ''; notice = ''; copying = true; res = []; try { res = await folderBackupRun(); } catch (e) { error = String(e); } finally { copying = false; } }
  async function toggleSubs(on: boolean) { try { await folderBackupSubmissionsSet(on); } catch (e) { error = String(e); } await load(); }
</script>

<section class="card">
  <h2>{tt('Copia en la nube')}</h2>
  <p class="muted">{tt('Guarda tus archivos de clase y tus entregas confirmadas en una carpeta que ya se sincronice con tu nube (')}<b>{tt('Google Drive para escritorio')}</b>{tt(', OneDrive, Dropbox, Nextcloud…), con la misma estructura que la biblioteca, dentro de')} <b>{tt('AulaDesk')}</b>{tt('. AulaDesk solo copia los archivos: no pide cuentas ni permisos, y nunca sobrescribe ni borra nada.')}</p>
  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

  {#if fs?.path}
    <p class="state"><i class="pulse-dot" style="--t: {fs.available ? 'var(--ok)' : 'var(--warn)'}"></i>{fs.path}{fs.available ? '' : ' (no disponible ahora)'}</p>
    <div class="autorow"><div><b>{tt('Copiar cada entrega confirmada')}</b><small class="muted">{tt('Cuando Moodle confirme una entrega (a mano, desde la app o programada), se copia a AulaDesk/…/Entregas.')}</small></div>
      <label class="row"><input type="checkbox" checked={fs.submissions} onchange={(e) => toggleSubs(e.currentTarget.checked)} />{tt('Activado')}</label></div>
    <div class="row">
      <button class="btn primary" disabled={copying || !fs.available} onclick={copyNow}>{copying ? 'Copiando…' : 'Copiar mis archivos ahora'}</button>
      <button class="btn" onclick={choose}>{tt('Cambiar carpeta')}</button>
      <button class="btn" onclick={clear}>{tt('Quitar')}</button>
    </div>
    {#if res.length}<p>{tt('{a} copiado(s), {b} ya estaban, {c} con error.', { a: n('copied'), b: n('skipped'), c: n('error') })}</p>
      {#if n('error')}<ul class="err-list">{#each res.filter((o) => o.status === 'error').slice(0, 8) as o}<li>{o.name || `#${o.file_id}`}: {o.detail}</li>{/each}</ul>{/if}{/if}
  {:else}
    <div class="row"><button class="btn primary" onclick={choose}>{tt('Elegir carpeta…')}</button></div>
  {/if}
</section>


<style>
  section { max-width: 780px; margin-bottom: 16px; } .row { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 10px; }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; word-break: break-all; }
  .autorow { display: flex; justify-content: space-between; align-items: center; gap: 20px; padding: 12px 0; border-top: 1px solid var(--line); flex-wrap: wrap; } .autorow > div:first-child { display: flex; flex-direction: column; gap: 2px; max-width: 520px; } .autorow small { font-size: 12.5px; }
  .err-list { color: var(--bad); font-size: 13px; }
</style>