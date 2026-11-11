<script lang="ts">
  import { t as tt } from '../lib/i18n.svelte';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Icon from '../lib/Icon.svelte';
  import { fam, refreshFamilias } from '../lib/familias.svelte';
  import { familiasOpen, familiasClose, familiasSync, familiasPause, familiasDisconnect, familiasRecon, getSetting, setSetting, type FamiliasSummary } from '../lib/api';
  import { fmtAgo } from '../lib/format';

  let error = $state(''); let syncing = $state(false); let summary = $state<FamiliasSummary | null>(null);
  let remember = $state(true); let startSync = $state(false);
  async function toggleStart() { startSync = !startSync; await run(() => setSetting('sync_start_familias', startSync ? '1' : '0')); } let confirmDisc = $state(false);
  let unlisten: UnlistenFn[] = [];
  const s = $derived(fam.status);
  const labels: Record<string, string> = $derived({ unknown: tt('Sin comprobar'), active: tt('Sesión iniciada'), expired: tt('Sesión caducada'), logged_out: tt('Sin sesión') });

  async function run(f: () => Promise<unknown>) { error = ''; try { await f(); await refreshFamilias(); } catch (e) { error = String(e); } }
  async function sync() { error = ''; summary = null; syncing = true; try { summary = await familiasSync(); } catch (e) { error = String(e); } finally { syncing = false; await refreshFamilias(); } }
  async function toggleRemember() { remember = !remember; await run(() => setSetting('remember_session_familias', remember ? '1' : '0')); }

  onMount(async () => {
    await refreshFamilias();
    remember = (await getSetting('remember_session_familias')) !== '0';
    startSync = (await getSetting('sync_start_familias')) === '1';
    unlisten.push(await listen('familias-session', refreshFamilias), await listen('familias-synced', refreshFamilias));
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<header class="fh"><div><span class="plat">{tt('Familias')}</span><h1>{tt('Conexión')}</h1></div></header>
{#if error}<p class="card err" role="alert">{error}</p>{/if}

<section class="card">
  <h2>{tt('Sesión')}</h2>
  <p class="state"><i class="pulse-dot" style="--t: {s?.session === 'active' ? 'var(--ok)' : 'var(--warn)'}"></i>{labels[s?.session ?? 'unknown']}</p>
  {#if s?.session === 'expired'}<p class="muted small" role="status">{tt('El portal cerró la sesión guardada (el servidor la cierra tras un tiempo sin usarla). Abre el portal e inicia sesión: mientras AulaDesk esté abierto, la sesión se mantiene activa y se vuelve a guardar sola.')}</p>{/if}
  <p class="muted">{tt('Inicias sesión tú en la ventana del portal; AulaDesk nunca ve ni guarda tu contraseña. Si tu acceso es con certificado digital o QR, se hace en esa misma ventana. Esta cuenta tiene su propio almacén de sesión, separado de Moodle.')}</p>
  <div class="row">
    <button class="btn primary" onclick={() => run(familiasOpen)}><Icon name="external" size={15} /> {tt('Abrir el portal para iniciar sesión')}</button>
    <button class="btn" onclick={() => run(familiasClose)}>{tt('Ocultar ventana')}</button>
  </div>
  <label class="remember"><input type="checkbox" checked={remember} onchange={toggleRemember} />
    <span>{tt('Mantener la sesión entre ejecuciones')}<br /><small class="muted">{tt('Las cookies de sesión se guardan en el llavero del sistema (nunca tu contraseña). Cuando el portal la cierre, tendrás que entrar de nuevo.')}</small></span></label>
  <label class="remember"><input type="checkbox" checked={startSync} disabled={!remember} onchange={toggleStart} />
    <span>{tt('Sincronizar al iniciar AulaDesk')}<br /><small class="muted">{tt('Al abrir la app, Familias se sincroniza sola (a la vez que Moodle si también lo tienes activado) y se muestra la pantalla de espera hasta que termine. Alarga el arranque. Necesita mantener la sesión entre ejecuciones y respeta el límite de una sincronización manual cada 5 minutos.')}</small></span></label>
</section>

<section class="card">
  <h2>{tt('Sincronización')}</h2>
  <p class="muted">{tt('Lee el alumnado, calificaciones, faltas y comunicaciones del portal con tu sesión, solo lectura. Para no cargar el portal: como máximo una sincronización automática por hora y una manual cada 5 minutos.')}</p>
  {#if s && s.recipes === 0}
    <p class="note" role="status"><Icon name="alert" size={15} /> <span><b>{tt('Todavía no se puede leer el portal.')}</b> {tt('Faltan las recetas de navegación y los extractores, que se escriben contra la estructura real de las pantallas tras un reconocimiento con tu cuenta. Hasta entonces no se sincroniza nada y no se muestra ningún dato inventado.')}</span></p>
  {/if}
  <div class="row">
    <button class="btn primary" disabled={syncing || s?.session !== 'active'} onclick={sync}>{#if !syncing}<Icon name="refresh" size={15} />{/if}{syncing ? 'Sincronizando…' : 'Sincronizar ahora'}</button>
    <label class="row"><input type="checkbox" checked={s?.paused} onchange={(e) => run(() => familiasPause(e.currentTarget.checked))} /> {tt('Pausar la sincronización automática')}</label>
  </div>
  <p class="muted small">{tt('Última correcta:')} <b>{s?.last_sync_at ? fmtAgo(s.last_sync_at) : 'nunca'}</b>{s?.last_error ? ` · último aviso: ${s.last_error}` : ''}</p>
  {#if summary}
    <p>{tt('{a} elementos leídos, {b} cambios.', { a: summary.seen, b: summary.changed })}</p>
    {#if summary.failed.length}<ul class="err-list">{#each summary.failed as f}<li>{f}</li>{/each}</ul>{/if}
  {/if}
</section>

<section class="card">
  <h2>{tt('Alumnado vinculado')}</h2>
  {#if s && s.students.length}
    <ul class="stu">{#each s.students as a (a.id)}<li><b>{a.display_name}</b><small class="muted">{[a.course_group, a.center, a.tutor ? `Tutor/a: ${a.tutor}` : ''].filter(Boolean).join(' · ')}</small></li>{/each}</ul>
  {:else}<p class="muted">{tt('Ninguno todavía. Aparecerá cuando la sincronización lea la lista de alumnos de tu cuenta.')}</p>{/if}
</section>

<section class="card">
  <h2>{tt('Privacidad y desvincular')}</h2>
  <p class="muted">{tt('Los datos de Familias (incluidos los de menores) viven solo en este equipo, en la base de datos cifrada, y nunca salen de él ni se copian a la nube. Desvincular cierra la sesión, borra las cookies y la entrada del llavero y elimina')} <b>{tt('todos')}</b> {tt('los datos de esta cuenta (alumnado, calificaciones, faltas, mensajes, eventos y avisos). No afecta a Moodle.')}</p>
  {#if !confirmDisc}<button class="btn danger" onclick={() => (confirmDisc = true)}>{tt('Desvincular cuenta de Familias…')}</button>
  {:else}<div class="row"><b>{tt('¿Seguro? Se borrarán los datos de esta cuenta.')}</b><button class="btn danger" onclick={() => { confirmDisc = false; run(familiasDisconnect); }}>{tt('Sí, desvincular y borrar')}</button><button class="btn" onclick={() => (confirmDisc = false)}>{tt('Cancelar')}</button></div>{/if}
  <details class="adv"><summary>{tt('Avanzado (desarrollo)')}</summary>
    <p class="muted small">{tt('Guarda en local un esqueleto de la página que muestre el portal, con el texto sustituido por su forma (sin nombres ni notas), para poder escribir los extractores. Se guarda en la carpeta de datos, en')} <code>{tt('cache/')}</code>.</p>
    <button class="btn sm" disabled={s?.session !== 'active'} onclick={() => run(familiasRecon)}>{tt('Guardar esqueleto de la página actual')}</button>
  </details>
</section>


<style>
  .fh { margin-bottom: 14px; } .fh h1 { margin: 0; } .plat { font-size: 11px; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; color: var(--accent); }
  section { max-width: 780px; margin-bottom: 16px; animation: rise .5s var(--ease) backwards; }
  .row { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin: 8px 0; }
  .state { display: inline-flex; align-items: center; gap: 10px; font-weight: 650; font-size: 15px; margin: 6px 0; }
  .remember { display: flex; gap: 12px; align-items: flex-start; margin: 14px 0 6px; } .remember small { font-size: 12px; } .small { font-size: 12.5px; }
  .note { display: flex; gap: 10px; align-items: flex-start; padding: 10px 14px; margin: 10px 0; border: 1px solid var(--line); border-left: 4px solid var(--warn); border-radius: var(--radius); background: var(--panel-2); font-size: 13px; }
  .err-list { color: var(--bad); } .stu { list-style: none; padding: 0; margin: 6px 0 0; display: grid; gap: 8px; } .stu li { display: flex; flex-direction: column; padding: 10px 12px; border: 1px solid var(--line); border-left: 4px solid var(--accent); border-radius: var(--radius); background: var(--panel-2); }
  .adv { margin-top: 16px; } .adv summary { cursor: pointer; color: var(--muted); } code { overflow-wrap: anywhere; }
</style>