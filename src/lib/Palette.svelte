<script lang="ts">
  import { trapFocus } from './a11y';
  import { t as tr } from './i18n.svelte';
  import { tick } from 'svelte';
  import { sections, shortcutSections } from './sections';
  import { fam, pickStudent } from './familias.svelte';
  import { nav, go } from './nav.svelte';
  import { listCourses, taskList } from './api';
  import Icon from './Icon.svelte';

  type Item = { key: string; label: string; hint: string; icon: string; run: () => void };
  let q = $state(''); let sel = $state(0); let input = $state<HTMLInputElement>();
  let courses = $state<{ id: number; name: string }[]>([]); let tasks = $state<{ id: number; name: string; course: string }[]>([]);

  $effect(() => {
    if (nav.palette) {
      q = ''; sel = 0;
      Promise.all([listCourses().catch(() => []), taskList().catch(() => [])]).then(([c, t]) => {
        courses = c.map((x) => ({ id: x.id, name: x.fullname })); tasks = t.map((x) => ({ id: x.id, name: x.name, course: x.course_name }));
      });
      tick().then(() => input?.focus());
    }
  });

  const norm = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '');
  const items = $derived.by<Item[]>(() => {
    const all: Item[] = [
      ...sections.filter((s) => !('needsStudents' in s && s.needsStudents) || (fam.status?.students.length ?? 0) > 0).map((s) => { const i = shortcutSections.findIndex((x) => x.id === s.id); return { key: 's' + s.id, label: s.group === 'core' ? tr(s.label) : `${s.group === 'moodle' ? 'Moodle' : 'Familias'} · ${tr(s.label)}`, hint: i >= 0 ? `Ir a · Ctrl+${i + 1}` : 'Ir a', icon: s.icon, run: () => go(s.id) }; }),
      ...(fam.status?.students ?? []).map((a) => ({ key: 'f' + a.id, label: a.display_name, hint: 'Familias · alumno', icon: 'cap', run: () => { pickStudent(a.id); go('f_home'); } })),
      ...courses.map((c) => ({ key: 'c' + c.id, label: c.name, hint: 'Asignatura', icon: 'courses', run: () => go('courses', { courseId: c.id }) })),
      ...tasks.map((t) => ({ key: 't' + t.id, label: t.name, hint: `Tarea · ${t.course}`, icon: 'tasks', run: () => go('tasks', { taskId: t.id }) })),
    ];
    const n = norm(q.trim());
    return (n ? all.filter((i) => norm(i.label + ' ' + i.hint).includes(n)) : all.slice(0, 12)).slice(0, 14);
  });
  $effect(() => { items; sel = 0; });

  const close = () => (nav.palette = false);
  function pick(i: Item) { close(); i.run(); }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); close(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); sel = (sel + 1) % Math.max(1, items.length); scrollSel(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); sel = (sel - 1 + items.length) % Math.max(1, items.length); scrollSel(); }
    else if (e.key === 'Enter' && items[sel]) { e.preventDefault(); pick(items[sel]); }
  }
  const scrollSel = () => tick().then(() => document.querySelector('.pal li.on')?.scrollIntoView({ block: 'nearest' }));
</script>

{#if nav.palette}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="veil" onclick={close}>
    <div class="pal glass" role="dialog" use:trapFocus tabindex="-1" aria-modal="true" aria-label={tr('Paleta de comandos')} onclick={(e) => e.stopPropagation()}>
      <div class="in"><Icon name="search" size={18} /><input bind:this={input} bind:value={q} onkeydown={key} placeholder={tr('Busca una sección, asignatura o tarea…')} aria-label={tr('Buscar')} /><kbd>Esc</kbd></div>
      <ul role="listbox">
        {#each items as it, i (it.key)}
          <li class:on={i === sel} role="option" aria-selected={i === sel}>
            <button onclick={() => pick(it)} onmousemove={() => (sel = i)}>
              <span class="ico"><Icon name={it.icon} size={16} /></span><span class="l">{it.label}</span><small class="muted">{it.hint}</small>
            </button>
          </li>
        {:else}<li class="none muted">{tr('Sin resultados para «{q}»', { q })}</li>{/each}
      </ul>
      <footer class="muted"><span><kbd>↑</kbd> <kbd>↓</kbd> moverse</span><span><kbd>Enter</kbd> abrir</span><span><kbd>Esc</kbd> cerrar</span></footer>
    </div>
  </div>
{/if}

<style>
  .veil { position: fixed; inset: 0; z-index: 100; background: rgba(0, 0, 0, .6); display: flex; justify-content: center; align-items: flex-start; padding-top: 14vh; animation: fade .18s backwards; }
  .pal { width: min(620px, 92vw); padding: 0; overflow: hidden; box-shadow: var(--shadow-lg), 0 0 0 1px var(--line-strong); background: var(--panel-solid); animation: pop .28s var(--ease-spring) backwards; }
  .in { display: flex; align-items: center; gap: 10px; padding: 14px 16px; border-bottom: 1px solid var(--line); color: var(--muted); }
  .in input { flex: 1; background: none !important; border: 0 !important; box-shadow: none !important; padding: 2px; font-size: 16px; color: var(--text); outline: none; }
  ul { list-style: none; margin: 0; padding: 8px; max-height: 52vh; overflow: auto; }
  li button { width: 100%; display: flex; align-items: center; gap: 12px; background: none; border: 0; border-radius: 10px; padding: 8px 10px; text-align: left; }
  li.on button { background: var(--accent-soft); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent); }
  .l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; } li small { white-space: nowrap; max-width: 45%; overflow: hidden; text-overflow: ellipsis; }
  .none { padding: 22px; text-align: center; }
  footer { display: flex; gap: 18px; padding: 9px 16px; border-top: 1px solid var(--line); font-size: 12px; }
</style>
