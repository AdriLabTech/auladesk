<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { trapFocus } from './a11y';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { pendingPromptData, scheduleCancel, scheduleMarkSeen, submissionSubmit, type PendingPrompt, type PendingItem, type SessionState } from './api';
  import { fmtDate, fmtSize } from './format';
  import { go } from './nav.svelte';
  import Icon from './Icon.svelte';
  import ScheduleForm from './ScheduleForm.svelte';

  let { session }: { session: SessionState } = $props();
  let data = $state<PendingPrompt | null>(null);
  let open = $state(false);
  let dismissed = false;             // «Más tarde» solo silencia lo que ya se mostró en esta ejecución
  let shownSig = '';
  let confirm = $state<number | null>(null); let agree = $state(false);
  let running = $state<{ id: number; text: string } | null>(null);
  let result = $state<{ id: number; ok: boolean; text: string } | null>(null);
  let scheduling = $state<PendingItem | null>(null);
  let error = $state('');
  let unlisten: UnlistenFn[] = [];

  const sig = (d: PendingPrompt) => d.items.map((i) => `${i.version.id}:${i.schedule?.id ?? 0}`).join(',') + '|' + d.results.map((r) => `${r.id}:${r.state}`).join(',');

  async function refresh(fromEvent = false) {
    try {
      const d = await pendingPromptData(); data = d;
      const s = sig(d);
      // Se muestra al entrar, y de nuevo solo si aparece algo nuevo (p. ej. el resultado de una entrega programada).
      if ((d.items.length || d.results.length) && (!dismissed || (fromEvent && s !== shownSig))) { open = true; shownSig = s; dismissed = false; }
      if (!d.items.length && !d.results.length) open = false;
    } catch { /* aún sin BD */ }
  }
  onMount(async () => {
    setTimeout(() => refresh(), 900);   // tras pintar la ventana, sin competir con la bienvenida
    unlisten.push(await listen('schedule-update', () => refresh(true)));
    unlisten.push(await listen<{ stage: string; text: string }>('submit-progress', (e) => { if (running) running = { ...running, text: e.payload.text }; }));
  });
  onDestroy(() => unlisten.forEach((u) => u()));

  async function close() { open = false; dismissed = true; confirm = null; agree = false; result = null; error = ''; try { await scheduleMarkSeen(); } catch { /* nada */ } }

  async function deliver(it: PendingItem) {
    if (!agree) return;
    const v = it.version; confirm = null; agree = false; error = ''; result = null; running = { id: v.id, text: 'Empezando…' };
    try { const r = await submissionSubmit(v.id, true, false); result = { id: v.id, ok: r.state === 'confirmed', text: r.state === 'confirmed' ? tt('Entrega confirmada por Moodle.') : tt('Enviada; Moodle aún no muestra un archivo idéntico (compruébalo en Entregas).') }; }
    catch (e) { result = { id: v.id, ok: false, text: String(e) }; }
    running = null; await refresh();
  }
  async function cancelSchedule(id: number) { error = ''; try { await scheduleCancel(id); await refresh(); } catch (e) { error = String(e); } }
  const dueText = (t: number | null) => (t ? `${tt('plazo')} ${fmtDate(t)}` : tt('sin fecha límite'));
  const stateText: Record<string, string> = $derived({ done: tt('Entregada'), failed: tt('No se pudo entregar'), missed: tt('Plazo perdido') });
</script>

{#if open && data}
  <div class="overlay" role="dialog" use:trapFocus aria-modal="true" aria-label={tt('Entregas preparadas')}>
    <div class="modal card">
      <header><span class="ico"><Icon name="submissions" size={20} /></span>
        <div><h2>{data.items.length ? tt(data.items.length === 1 ? 'Tienes {n} entrega preparada' : 'Tienes {n} entregas preparadas', { n: data.items.length }) : tt('Resultado de entregas programadas')}</h2>
          <p class="muted small">{tt('Nada se envía a Moodle sin que lo confirmes aquí o lo hayas programado antes.')}</p></div></header>

      {#if session !== 'active' && session !== 'unknown' && data.items.length}<p class="msg err" role="alert">{tt('No hay sesión de Moodle activa. Inicia sesión en «Moodle / Conexión» para poder entregar; una entrega programada también la necesita.')}</p>{/if}

      {#if data.results.length}
        <section><h3>{tt('Entregas programadas')}</h3>
          {#each data.results as r (r.id)}
            <div class="res {r.state}"><b>{stateText[r.state] ?? r.state}</b> · {r.activity_name} <span class="muted">({r.course_name})</span>{#if r.last_error}<br /><small class="muted">{r.last_error}</small>{/if}</div>
          {/each}
        </section>
      {/if}

      {#if data.items.length}
        <ul>
          {#each data.items as it (it.version.id)}
            {@const v = it.version}
            <li>
              <div class="head"><b>{v.activity_name}</b><span class="muted">{v.course_name} · {dueText(v.deadline_at)}</span></div>
              <p class="file">{v.file_name} <span class="muted">· {fmtSize(v.size_bytes)} · v{v.version} · <code title={v.sha256}>{v.sha256.slice(0, 12)}…</code></span></p>
              {#if it.can_submit === 0}<p class="msg err small">{tt('Moodle indica que ya no se puede entregar (plazo cerrado o sin permiso). Sincroniza para actualizar.')}</p>{/if}
              {#if it.schedule}
                <p class="sched"><Icon name="calendar" size={14} /> {tt('Programada para')} <b>{fmtDate(it.schedule.run_at)}</b>{it.schedule.last_error ? ` · ${it.schedule.last_error}` : ''}
                  {#if it.schedule.state === 'scheduled'}<button class="btn sm" onclick={() => cancelSchedule(it.schedule!.id)}>{tt('Cancelar programación')}</button>{/if}</p>
              {/if}
              {#if running?.id === v.id}<p class="muted small" role="status"><i class="spin"></i> {running.text}</p>
              {:else if result?.id === v.id}<p class="msg {result.ok ? 'ok' : 'err'}" role="status">{result.text}</p>
              {:else if confirm === v.id}
                <label class="agree"><input type="checkbox" bind:checked={agree} /> {tt('Entiendo que esto')} <b>{tt('envía «{f}» a Moodle ahora', { f: v.file_name })}</b> {tt('y lo verá el profesorado.')}</label>
                <div class="acts"><button class="btn primary sm" disabled={!agree || session !== 'active'} onclick={() => deliver(it)}>{tt('Confirmar y entregar ahora')}</button><button class="btn sm" onclick={() => { confirm = null; agree = false; }}>{tt('Atrás')}</button></div>
              {:else}
                <div class="acts">
                  <button class="btn primary sm" disabled={running !== null || it.can_submit === 0} onclick={() => { confirm = v.id; agree = false; }}>{tt('Entregar ahora…')}</button>
                  {#if !it.schedule}<button class="btn sm" disabled={running !== null || it.can_submit === 0} onclick={() => (scheduling = it)}>{tt('Programar…')}</button>{/if}
                  <button class="btn sm" onclick={() => { close(); go('submissions'); }}>{tt('Ver en Entregas')}</button>
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      {#if error}<p class="msg err" role="alert">{error}</p>{/if}
      <div class="acts end"><button class="btn" disabled={running !== null} onclick={close}>{data.items.length ? 'Más tarde' : 'Cerrar'}</button></div>
    </div>
  </div>
{/if}
{#if scheduling}<ScheduleForm version={scheduling.version} onCancel={() => (scheduling = null)} onDone={() => { scheduling = null; refresh(); }} />{/if}


<style>
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, .6); display: grid; place-items: center; z-index: 80; padding: 20px; animation: fade .2s backwards; }
  .modal { width: min(640px, 100%); max-height: 86vh; overflow: auto; background: var(--panel-solid); box-shadow: var(--shadow-lg); animation: pop .3s var(--ease-spring) backwards; }
  header { display: flex; gap: 14px; align-items: flex-start; margin-bottom: 8px; } header h2 { margin: 0; } header p { margin: 2px 0 0; }
  .ico { width: 38px; height: 38px; border-radius: var(--radius); display: grid; place-items: center; background: var(--accent); color: var(--accent-fg); flex: none; }
  .small { font-size: 12.5px; } h3 { font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: var(--muted); margin: 14px 0 6px; }
  ul { list-style: none; margin: 10px 0 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
  li { border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: var(--radius); padding: 12px 14px; background: var(--panel-2); }
  .head { display: flex; flex-direction: column; gap: 1px; } .file { margin: 6px 0 2px; overflow-wrap: anywhere; }
  .sched { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin: 6px 0; font-size: 13px; }
  .res { padding: 8px 12px; border: 1px solid var(--line); border-radius: var(--radius); margin-bottom: 6px; } .res.done { border-left: 3px solid var(--ok); } .res.failed, .res.missed { border-left: 3px solid var(--bad); }
  .agree { display: flex; gap: 10px; align-items: flex-start; margin: 10px 0 4px; font-size: 13.5px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; } .acts.end { justify-content: flex-end; margin-top: 16px; }
  .spin { display: inline-block; width: 12px; height: 12px; border: 2px solid var(--accent); border-right-color: transparent; border-radius: 50%; animation: spin .8s linear infinite; vertical-align: -2px; }
</style>