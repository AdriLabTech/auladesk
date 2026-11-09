<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Icon from '../lib/Icon.svelte';
  import DriveCard from '../lib/DriveCard.svelte';
  import { account, initials } from '../lib/account.svelte';
  import { moodleOpen, moodleClose, moodleStatus, moodleSync, moodleForgetSession, getSetting, setSetting, autosyncStatus, type SessionState, type SyncSummary, type AutoSyncStatus } from '../lib/api';
  import { fmtAgo, fmtRemaining } from '../lib/format';

  let { onSession }: { onSession: (s: SessionState) => void } = $props();
  let sess = $state<SessionState>('unknown');
  let error = $state('');
  let remember = $state(true);
  let syncing = $state(false);
  let progress = $state<{ done: number; total: number; current: string | null } | null>(null);
  let summary = $state<SyncSummary | null>(null);
  let unlisten: UnlistenFn[] = [];
  let auto = $state<AutoSyncStatus | null>(null);
  const intervals = [10, 15, 30, 60, 120, 240, 360, 720];
  const minLabel = (m: number) => (m < 60 ? `${m} min` : `${m / 60} h`);
  async function loadAuto() { try { auto = await autosyncStatus(); } catch { /* sin BD aún */ } }
  async function setAuto(key: string, value: string) { await run(() => setSetting(key, value)); await loadAuto(); }

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
    sess = await moodleStatus(); loadAuto();
    const tick = setInterval(loadAuto, 20000); unlisten.push(() => clearInterval(tick));
    unlisten.push(await listen('autosync', loadAuto));
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

<section class="card">
  <h2>Sincronización automática</h2>
  <p class="muted">AulaDesk vuelve a leer Moodle cada cierto tiempo mientras la app esté en marcha y avisa de tareas nuevas, cambios de fecha y notas. Necesita la sesión de Moodle iniciada.</p>
  {#if auto}
    <div class="astat" role="status">
      {#if auto.running}<i class="pulse-dot" style="--t: var(--accent)"></i><b>Sincronizando ahora…</b>
      {:else if !auto.enabled}<span class="muted">Desactivada. Solo se sincroniza cuando tú lo pides.</span>
      {:else if !auto.session_active}<i class="pulse-dot" style="--t: var(--warn)"></i><span>Esperando a que inicies sesión en Moodle.</span>
      {:else}
        <i class="pulse-dot" style="--t: {auto.last_ok === false ? 'var(--warn)' : 'var(--ok)'}"></i>
        <span>Última: <b>{auto.last_at ? fmtAgo(auto.last_at) : 'nunca'}</b>{auto.last_ok === false ? ' (con errores)' : ''}</span>
        <span class="sep">·</span><span>Próxima: <b>{auto.next_at ? (auto.next_at <= Date.now() / 1000 ? 'en breve' : fmtRemaining(auto.next_at - Math.floor(Date.now() / 1000))) : 'en breve'}</b></span>
      {/if}
    </div>
    <div class="autorow"><div><b>Sincronizar automáticamente</b><small class="muted">Recomendado: mantiene al día tus tareas y fechas sin que hagas nada.</small></div>
      <label class="row"><input type="checkbox" checked={auto.enabled} onchange={(e) => setAuto('autosync_enabled', e.currentTarget.checked ? '1' : '0')} /> Activada</label></div>
    <div class="autorow"><div><b>Cada</b><small class="muted">Si una sincronización falla, se reintenta a la mitad de este tiempo.</small></div>
      <select value={String(auto.minutes)} disabled={!auto.enabled} onchange={(e) => setAuto('autosync_minutes', e.currentTarget.value)} aria-label="Intervalo">{#each intervals as m}<option value={String(m)}>{minLabel(m)}</option>{/each}</select></div>
    <div class="autorow"><div><b>Al cerrar la ventana, seguir en la bandeja</b><small class="muted">{auto.tray ? 'La app se oculta en la bandeja del sistema y sigue sincronizando. Se cierra del todo desde su menú.' : 'Bandeja no disponible en este sistema (en Linux hace falta libayatana-appindicator). Al cerrar la ventana se cierra la app.'}</small></div>
      <label class="row"><input type="checkbox" checked={auto.close_to_tray} disabled={!auto.tray} onchange={(e) => setAuto('close_to_tray', e.currentTarget.checked ? '1' : '0')} /> Activado</label></div>
  {/if}
</section>

<DriveCard />

<style>
  .autorow { display: flex; justify-content: space-between; align-items: center; gap: 20px; padding: 12px 0; border-top: 1px solid var(--line); flex-wrap: wrap; } .autorow:first-of-type { border-top: 0; margin-top: 6px; }
  .autorow > div:first-child { display: flex; flex-direction: column; gap: 2px; max-width: 520px; } .autorow small { font-size: 12.5px; }
  .astat { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 10px 12px; margin: 10px 0 2px; background: var(--panel-2); border: 1px solid var(--line); border-left: 4px solid var(--accent); border-radius: var(--radius); font-size: 13px; }
  .astat .sep { color: var(--faint); }
  .remember { display: flex; gap: 12px; align-items: flex-start; margin: 16px 0 12px; } .remember small { font-size: 12px; }
  section { max-width: 780px; margin-bottom: 16px; animation: rise .5s var(--ease) backwards; } section:nth-of-type(2) { animation-delay: .08s; }
  .err-list { color: var(--bad); } .row { display: flex; gap: 10px; flex-wrap: wrap; }
  .who { display: flex; align-items: center; gap: 14px; padding: 12px 14px; margin: 8px 0 4px; border: 1px solid var(--line); border-left: 4px solid var(--accent); border-radius: var(--radius); background: var(--panel-2); } .who div { display: flex; flex-direction: column; } .who b { font-family: var(--font-display); font-size: 17px; }
  .avatar { width: 40px; height: 40px; border-radius: var(--radius); display: grid; place-items: center; background: var(--accent); color: var(--accent-fg); font: 800 15px/1 var(--font-display); }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; }
  .prog { height: 6px; border-radius: 99px; background: var(--panel-3); overflow: hidden; margin: 12px 0 4px; } .prog span { display: block; height: 100%; background: var(--accent); border-radius: 99px; transition: width .4s var(--ease); }
</style>
