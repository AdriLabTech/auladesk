<script lang="ts">
  import { onMount } from 'svelte';
  import { notifList, notifMarkRead, notifSettingsGet, notifSettingsSet, notifTest, serviceStatus, serviceInstall, serviceUninstall, type NotifRow, type NotifSettings, type ServiceStatus } from '../lib/api';
  import { fmtDate } from '../lib/format';
  import Icon from '../lib/Icon.svelte';

  let { onChange = () => {} }: { onChange?: () => void } = $props();
  let tab = $state<'inbox' | 'prefs'>('inbox');
  let rows = $state<NotifRow[]>([]);
  let s = $state<NotifSettings | null>(null);
  let svc = $state<ServiceStatus | null>(null);
  let error = $state(''); let notice = $state('');

  const cats: Record<string, string> = { deadline: 'Fechas límite de tareas', exam: 'Exámenes', reminder: 'Recordatorios personales', new_activity: 'Tareas, cuestionarios y recursos nuevos', date_changed: 'Cambios de fecha', grade: 'Nuevas calificaciones', submission: 'Entregas confirmadas', sync_error: 'Errores de sincronización', download: 'Documentos descargados', digest: 'Resúmenes diario y semanal' };
  const glyph: Record<string, string> = { deadline: 'clock', exam: 'edit', reminder: 'bell', new_activity: 'plus', date_changed: 'refresh', grade: 'star', submission: 'check', sync_error: 'alert', download: 'download', digest: 'inbox' };
  const days = ['Lun', 'Mar', 'Mié', 'Jue', 'Vie', 'Sáb', 'Dom'];
  const leadOptions = [14, 7, 5, 3, 2, 1];

  async function load() { try { rows = await notifList(150); s = await notifSettingsGet(); svc = await serviceStatus(); } catch (e) { error = String(e); } }
  onMount(() => { load(); const t = setInterval(() => { if (tab === 'inbox') load(); }, 15000); return () => clearInterval(t); });
  async function run(f: () => Promise<unknown>, ok = '') { error = ''; notice = ''; try { await f(); if (ok) notice = ok; await load(); onChange(); } catch (e) { error = String(e); } }
  async function save() { if (s) await run(async () => { s = await notifSettingsSet(s!); }, 'Preferencias guardadas'); }
  const toggleLead = (d: number) => { if (!s) return; s.lead_days = s.lead_days.includes(d) ? s.lead_days.filter((x) => x !== d) : [...s.lead_days, d].sort((a, b) => b - a); };
  const unread = $derived(rows.filter((r) => !r.read_at).length);
</script>

<div><h1>Notificaciones</h1>
<div class="seg" role="group" aria-label="Sección">
  <button class:on={tab === 'inbox'} onclick={() => (tab = 'inbox')}>Bandeja {unread ? `(${unread})` : ''}</button>
  <button class:on={tab === 'prefs'} onclick={() => (tab = 'prefs')}>Preferencias</button>
</div>
{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

{#if tab === 'inbox'}
  <div class="acts">
    <button class="btn sm" onclick={() => run(() => notifMarkRead())}>Marcar todas como leídas</button>
    <button class="btn sm" onclick={() => run(notifTest, 'Prueba enviada al escritorio')}>Enviar notificación de prueba</button>
  </div>
  {#each rows as n (n.id)}
    <button class="n" class:unread={!n.read_at} onclick={() => run(() => notifMarkRead(n.id))}>
      <span class="g p{n.priority}" aria-hidden="true"><Icon name={glyph[n.category] ?? 'file'} size={16} /></span>
      <span class="t"><b>{n.title}</b>{#if n.body}<small>{n.body}</small>{/if}<em>{cats[n.category] ?? n.category} · {fmtDate(n.created_at)}</em></span>
    </button>
  {:else}
    <div class="card"><p>No hay notificaciones.</p><p class="muted">Aparecerán avisos de fechas límite, cambios de fecha, calificaciones nuevas y recordatorios.</p></div>
  {/each}
{:else if s}
  <section class="card"><h2>Cuándo avisar</h2>
    <p class="muted">Avisos de fechas límite y exámenes, con X días de antelación. Si entregas la tarea, sus avisos se cancelan solos; si la fecha cambia, se recalculan.</p>
    <div class="chips">{#each leadOptions as d}<button class="chip" class:on={s.lead_days.includes(d)} onclick={() => toggleLead(d)}>{d} {d === 1 ? 'día' : 'días'}</button>{/each}</div>
    <div class="row"><label class="f">No avisar desde<input type="time" bind:value={s.quiet_start} /></label><label class="f">hasta<input type="time" bind:value={s.quiet_end} /></label></div>
    <p class="muted small">Franja silenciosa: lo pendiente se aplaza hasta que termine, no se pierde.</p>
    <div class="chips">{#each days as d, i}<button class="chip" class:on={s.weekdays[i]} onclick={() => (s!.weekdays[i] = !s!.weekdays[i])} aria-pressed={s.weekdays[i]}>{d}</button>{/each}</div>
    <label class="chk"><input type="checkbox" bind:checked={s.muted} /> Silenciar (no se muestran avisos en el escritorio; siguen en la bandeja)</label>
  </section>
  <section class="card"><h2>Resúmenes</h2>
    <label class="chk"><input type="checkbox" checked={s.daily_digest !== null} onchange={(e) => (s!.daily_digest = e.currentTarget.checked ? '08:30' : null)} /> Resumen diario</label>
    {#if s.daily_digest !== null}<label class="f">Hora<input type="time" bind:value={s.daily_digest} /></label>{/if}
    <label class="chk"><input type="checkbox" checked={s.weekly_digest !== null} onchange={(e) => (s!.weekly_digest = e.currentTarget.checked ? '09:00' : null)} /> Resumen semanal</label>
    {#if s.weekly_digest !== null}<div class="row"><label class="f">Día<select bind:value={s.weekly_day}>{#each days as d, i}<option value={i}>{d}</option>{/each}</select></label><label class="f">Hora<input type="time" bind:value={s.weekly_digest} /></label></div>{/if}
  </section>
  <section class="card"><h2>Categorías</h2>
    {#each Object.entries(cats) as [k, label]}<label class="chk"><input type="checkbox" checked={s.categories[k] ?? true} onchange={(e) => (s!.categories[k] = e.currentTarget.checked)} /> {label}</label>{/each}
  </section>
  <button class="btn primary" onclick={save}>Guardar preferencias</button>

  <section class="card svc"><h2>Avisos con la app cerrada</h2>
    <p class="muted">Un servicio de usuario (<code>systemd --user</code>) ejecuta AulaDesk sin ventana y solo lee tu base de datos local, nunca Moodle. Es opcional y no se instala sin que lo pidas.</p>
    {#if svc}<p>Estado: <b>{svc.installed ? (svc.active ? 'instalado y activo' : 'instalado, parado') : 'no instalado'}</b></p>
      {#if svc.installed}<button class="btn" onclick={() => run(serviceUninstall, 'Servicio desinstalado')}>Desinstalar servicio</button>
      {:else}<button class="btn" onclick={() => run(serviceInstall, 'Servicio instalado y arrancado')}>Instalar servicio en segundo plano</button>{/if}
      <p class="muted small">Archivo: <code>{svc.unit_path}</code></p>{/if}
  </section>
{/if}
</div>

<style>
  .seg { margin: 14px 0 16px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 14px; }
  .n { display: flex; gap: 14px; width: 100%; text-align: left; background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 12px 16px; margin-bottom: 8px; color: var(--text); max-width: 880px; box-shadow: var(--shadow); animation: rise .45s var(--ease) backwards; transition: transform .2s var(--ease), border-color .2s; }
  .n:hover { transform: translateX(4px); border-color: var(--line-strong); }
  .n.unread { border-left: 3px solid var(--accent); background: color-mix(in srgb, var(--accent) 7%, var(--panel)); } .n:not(.unread) { opacity: .7; }
  .g { width: 34px; height: 34px; border-radius: 10px; display: grid; place-items: center; background: var(--panel-2); flex: none; color: var(--accent); }
  .g.p2 { background: color-mix(in srgb, var(--bad) 18%, transparent); color: var(--bad); } .g.p1 { background: color-mix(in srgb, var(--warn) 18%, transparent); color: var(--warn); }
  .t { display: flex; flex-direction: column; gap: 2px; min-width: 0; } .t b { overflow-wrap: anywhere; font-weight: 600; } .t small { color: var(--muted); white-space: pre-line; } .t em { font-style: normal; font-size: 11.5px; color: var(--faint); }
  section { max-width: 780px; margin-bottom: 16px; animation: rise .5s var(--ease) backwards; } .chips { margin: 10px 0; }
  .row { display: flex; gap: 14px; flex-wrap: wrap; }
  .chk { display: flex; gap: 10px; align-items: center; margin: 8px 0; font-size: 13.5px; } .small { font-size: 12px; } code { overflow-wrap: anywhere; }
</style>
