<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Icon from '../lib/Icon.svelte';
  import { account, initials } from '../lib/account.svelte';
  import { moodleOpen, moodleClose, moodleStatus, moodleSync, moodleForgetSession, getSetting, setSetting, type SessionState, type SyncSummary } from '../lib/api';

  let { onSession }: { onSession: (s: SessionState) => void } = $props();
  let sess = $state<SessionState>('unknown');
  let error = $state('');
  let remember = $state(true);
  let syncing = $state(false);
  let progress = $state<{ done: number; total: number; current: string | null } | null>(null);
  let summary = $state<SyncSummary | null>(null);
  let unlisten: UnlistenFn[] = [];

  const labels: Record<SessionState, string> = {
    unknown: 'Sin comprobar', active: 'Sesión iniciada', expired: 'Sesión caducada', logged_out: 'Sin sesión',
  };

  async function run(f: () => Promise<void>) { error = ''; try { await f(); } catch (e) { error = String(e); } }

  async function sync() {
    error = ''; summary = null; syncing = true;
    try { summary = await moodleSync(); } catch (e) { error = String(e); } finally { syncing = false; }
  }

  async function toggleRemember() {
    remember = !remember;
    await run(() => setSetting('remember_session', remember ? '1' : '0'));
  }
  async function forget() { await run(moodleForgetSession); }

  onMount(async () => {
    remember = (await getSetting('remember_session')) !== '0';
    unlisten.push(await listen<typeof progress>('sync-progress', (e) => (progress = e.payload)));
    sess = await moodleStatus();
    unlisten.push(await listen<SessionState>('moodle-session', (e) => { sess = e.payload; onSession(e.payload); }));
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<h1>Moodle / Conexión</h1>
{#if error}<p class="card err" role="alert">{error}</p>{/if}
<section class="card">
  <h2>Sesión</h2>
  {#if account.name}
    <div class="who"><span class="avatar" aria-hidden="true">{initials(account.name)}</span><div><b>{account.name}</b><small class="muted">Cuenta de Moodle{account.userId ? ` · ID ${account.userId}` : ''} · educacionadistancia.juntadeandalucia.es</small></div></div>
  {:else if sess === 'active'}
    <p class="muted small">Sesión iniciada, pero no se ha podido leer el nombre de la cuenta.</p>
  {/if}
  <p class="state"><i class="pulse-dot" style="--t: {sess === 'active' ? 'var(--ok)' : 'var(--warn)'}"></i>{labels[sess]}</p>
  <p class="muted">Inicias sesión en la ventana de Moodle con tus credenciales; AulaDesk nunca ve la contraseña. La sesión se conserva en el almacén propio del navegador integrado.</p>
  <div class="row">
    <button class="btn primary" onclick={() => run(moodleOpen)}><Icon name="external" size={15} /> Abrir Moodle</button>
    <button class="btn" onclick={() => run(moodleClose)}>Ocultar ventana</button>
  </div>
  <label class="remember"><input type="checkbox" checked={remember} onchange={toggleRemember} />
    <span>Mantener la sesión iniciada entre ejecuciones<br /><small class="muted">Las cookies de sesión se guardan cifradas en el llavero del sistema (nunca tu contraseña). El servidor cierra la sesión tras 8 h sin usarla; entonces tendrás que entrar de nuevo.</small></span>
  </label>
  <button class="btn" onclick={forget}>Cerrar sesión y olvidarla</button>
</section>

<section class="card">
  <h2>Sincronización</h2>
  <p class="muted">Lee tus asignaturas, temas y actividades desde Moodle (solo lectura). Necesita la ventana de Moodle abierta con la sesión iniciada.</p>
  <button class="btn primary" disabled={syncing || sess !== 'active'} onclick={sync}>{#if !syncing}<Icon name="refresh" size={15} />{/if}{syncing ? 'Sincronizando…' : 'Sincronizar ahora'}</button>
  {#if syncing && progress}<div class="prog" role="progressbar" aria-valuenow={progress.done} aria-valuemax={progress.total}><span style="width: {progress.total ? (progress.done / progress.total) * 100 : 0}%"></span></div><p class="muted small">{progress.done}/{progress.total}{progress.current ? ` · ${progress.current}` : ''}</p>{/if}
  {#if summary}
    <p>{summary.synced} de {summary.courses} asignaturas y {summary.assignments} tareas sincronizadas.</p>
    {#if summary.failed.length}<ul class="err-list">{#each summary.failed as f}<li>{f}</li>{/each}</ul>{/if}
  {/if}
</section>

<style>
  .remember { display: flex; gap: 12px; align-items: flex-start; margin: 16px 0 12px; } .remember small { font-size: 12px; }
  section { max-width: 780px; margin-bottom: 16px; animation: rise .5s var(--ease) backwards; } section:nth-of-type(2) { animation-delay: .08s; }
  .err-list { color: var(--bad); } .row { display: flex; gap: 10px; flex-wrap: wrap; }
  .who { display: flex; align-items: center; gap: 14px; padding: 12px 14px; margin: 8px 0 4px; border: 1px solid var(--line); border-left: 4px solid var(--accent); border-radius: var(--radius); background: var(--panel-2); } .who div { display: flex; flex-direction: column; } .who b { font-family: var(--font-display); font-size: 17px; }
  .avatar { width: 40px; height: 40px; border-radius: var(--radius); display: grid; place-items: center; background: var(--accent); color: var(--accent-fg); font: 800 15px/1 var(--font-display); }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; }
  .prog { height: 6px; border-radius: 99px; background: var(--panel-3); overflow: hidden; margin: 12px 0 4px; } .prog span { display: block; height: 100%; background: var(--accent); border-radius: 99px; transition: width .4s var(--ease); }
</style>
