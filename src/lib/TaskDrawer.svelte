<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { trapFocus } from './a11y';
  import type { TaskRow } from './api';
  import { stateInfo } from './format';
  import Icon from './Icon.svelte';
  import TaskDetail from '../views/TaskDetail.svelte';

  /** Cajón lateral con el detalle completo de una tarea (descripción, entrega…). Esc o clic fuera para cerrar. */
  let { task, onClose, onChange = () => {} }: { task: TaskRow; onClose: () => void; onChange?: () => void | Promise<void> } = $props();
  const stale = $derived(task.state === 'fuera_de_plazo' && task.seconds_to_due !== null && task.seconds_to_due <= -30 * 86400);
  const si = $derived(stateInfo[task.state]);
  function key(e: KeyboardEvent) { if (e.key === 'Escape') onClose(); }
</script>

<svelte:window onkeydown={key} />
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onClose}></div>
<div class="drawer" role="dialog" use:trapFocus tabindex="-1" aria-label={`Detalle: ${task.name}`}>
  <header>
    <div>
      <span class="muted small">{task.course_name}{task.section_position !== null ? ` · ${task.section_position === 0 ? tt('General') : tt('Tema {n}', { n: task.section_position })}` : ''}</span>
      <h2>{task.name}</h2>
      <span class="badges"><span class="badge b-{si.tone}">{stale ? 'Antigua · sin entregar' : si.label}</span>{#if task.grade}<span class="grade">{task.grade}</span>{/if}</span>
    </div>
    <button class="btn ghost icon" onclick={onClose} aria-label={tt('Cerrar detalle')}><Icon name="x" size={18} /></button>
  </header>
  <div class="drawer-body"><TaskDetail t={task} {onChange} /></div>
</div>


<style>
  .scrim { position: fixed; inset: 0; z-index: 60; background: rgba(0, 0, 0, .45); animation: fade .2s backwards; }
  .drawer { position: fixed; z-index: 61; top: 0; right: 0; bottom: 0; width: min(600px, 94vw); display: flex; flex-direction: column; background: var(--panel-solid); border-left: 1px solid var(--line-strong); box-shadow: var(--shadow-lg); animation: slide .35s var(--ease) backwards; }
  @keyframes slide { from { transform: translateX(40px); opacity: 0; } to { transform: none; opacity: 1; } }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; padding: 20px 22px 14px; border-bottom: 1px solid var(--line); }
  header > div { display: flex; flex-direction: column; gap: 6px; align-items: flex-start; min-width: 0; }
  h2 { font-family: var(--font-display); font-size: 21px; letter-spacing: -.02em; line-height: 1.2; overflow-wrap: anywhere; } .badges { display: flex; gap: 8px; align-items: center; }
  .badge { --t: var(--t-badge, var(--c-muted)); } .b-green { --t-badge: var(--c-green); } .b-red { --t-badge: var(--c-red); } .b-teal { --t-badge: var(--c-teal); } .b-violet { --t-badge: var(--c-violet); } .b-blue { --t-badge: var(--c-blue); } .b-muted { --t-badge: var(--c-muted); }
  .grade { font-size: 12px; font-weight: 700; color: var(--ok); padding: 2px 9px; border-radius: 4px; background: color-mix(in srgb, var(--ok) 14%, transparent); border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent); }
  .drawer-body { flex: 1; overflow-y: auto; padding: 4px 22px 28px; }
</style>