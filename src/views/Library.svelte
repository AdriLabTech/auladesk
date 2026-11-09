<script lang="ts">
  import { onMount } from 'svelte';
  import { open as pickFile } from '@tauri-apps/plugin-dialog';
  import { listFiles, listDuplicates, listCourses, listFolders, fileOpen, fileReveal, fileRename, fileMove, folderCreate, fileDelete, importFile, type FileRow, type CourseRow } from '../lib/api';
  import { fmtSize, fmtDate } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { openViewer } from '../lib/viewerstate.svelte';

  let files = $state<FileRow[]>([]);
  let courses = $state<CourseRow[]>([]);
  let folders = $state<string[]>([]);
  let dupGroups = $state<FileRow[][]>([]);
  let q = $state(''); let courseId = $state(''); let kind = $state(''); let sort = $state('date'); let group = $state<'course' | 'kind' | 'month' | 'flat'>('course');
  let showDups = $state(false);
  let error = $state(''); let notice = $state('');
  let editing = $state<{ id: number; mode: 'rename' | 'move'; value: string } | null>(null);
  let confirmDel = $state<number | null>(null);
  let newFolder = $state('');

  const kinds = ['pdf', 'documento', 'hoja', 'presentación', 'comprimido', 'imagen', 'código', 'otro'];
  const kindGlyph: Record<string, string> = { pdf: 'PDF', documento: 'DOC', hoja: 'XLS', 'presentación': 'PPT', comprimido: 'ZIP', imagen: 'IMG', 'código': '</>', otro: 'FILE' };

  async function load() {
    try {
      files = await listFiles({ q, course_id: courseId ? Number(courseId) : null, kind, sort });
      dupGroups = await listDuplicates(); folders = await listFolders();
    } catch (e) { error = String(e); }
  }
  onMount(async () => { courses = await listCourses().catch(() => []); await load(); });
  $effect(() => { q; courseId; kind; sort; load(); });

  async function act(f: () => Promise<unknown>, ok = '') {
    error = ''; notice = '';
    try { await f(); if (ok) notice = ok; editing = null; confirmDel = null; await load(); } catch (e) { error = String(e); }
  }
  async function submitEdit() {
    if (!editing) return; const e = editing;
    await act(() => (e.mode === 'rename' ? fileRename(e.id, e.value) : fileMove(e.id, e.value)), e.mode === 'rename' ? 'Renombrado' : 'Movido');
  }
  async function doImport() {
    const p = await pickFile({ multiple: false, directory: false });
    if (typeof p === 'string') await act(() => importFile(p), 'Archivo importado a «Sin_clasificar»');
  }

  const grouped = $derived.by(() => {
    const m = new Map<string, FileRow[]>();
    for (const f of files) {
      const key = group === 'course' ? f.course_name ?? 'Sin asignatura' : group === 'kind' ? f.kind : group === 'month' ? cap(new Date(f.downloaded_at * 1000).toLocaleDateString('es-ES', { month: 'long', year: 'numeric' })) : 'Todos los archivos';
      (m.get(key) ?? m.set(key, []).get(key)!).push(f);
    }
    return [...m.entries()];
  });
  const cap = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);
  const total = $derived(files.reduce((a, f) => a + f.size_bytes, 0));
</script>

<h1>Biblioteca de archivos</h1>
<p class="muted">{files.length} archivo(s) · {fmtSize(total)} · guardados en tu equipo, disponibles sin conexión.</p>

<div class="bar">
  <input type="search" placeholder="Buscar por nombre, ruta o actividad…" bind:value={q} aria-label="Buscar archivos" />
  <select bind:value={courseId} aria-label="Asignatura"><option value="">Todas las asignaturas</option>{#each courses as c}<option value={String(c.id)}>{c.fullname}</option>{/each}</select>
  <select bind:value={kind} aria-label="Tipo"><option value="">Todos los tipos</option>{#each kinds as k}<option value={k}>{k}</option>{/each}</select>
  <select bind:value={sort} aria-label="Orden"><option value="date">Más recientes</option><option value="name">Nombre</option><option value="size">Tamaño</option><option value="type">Tipo</option></select>
  <select bind:value={group} aria-label="Agrupar"><option value="course">Agrupar: asignatura</option><option value="kind">Agrupar: tipo</option><option value="month">Agrupar: fecha de descarga</option><option value="flat">Sin agrupar</option></select>
</div>
<div class="bar2">
  <button class="btn" onclick={doImport}><Icon name="plus" size={15} /> Importar archivo propio…</button>
  <form onsubmit={(e) => { e.preventDefault(); if (newFolder.trim()) act(() => folderCreate(newFolder), `Carpeta «${newFolder}» creada`).then(() => (newFolder = '')); }}>
    <input placeholder="Nueva carpeta (p. ej. Personal/Apuntes)" bind:value={newFolder} aria-label="Nombre de carpeta" /><button class="btn">Crear</button>
  </form>
  {#if dupGroups.length}<button class="btn warn" onclick={() => (showDups = !showDups)}>{dupGroups.length} grupo(s) duplicado(s)</button>{/if}
</div>

{#if error}<p class="msg err" role="alert">{error}</p>{/if}
{#if notice}<p class="msg ok" role="status">{notice}</p>{/if}

{#if showDups}
  <section class="card dups"><h2>Archivos con el mismo contenido (SHA-256)</h2>
    {#each dupGroups as g}<div class="dg">{#each g as f}<div><code>{f.rel_path}</code> <span class="muted">{fmtSize(f.size_bytes)}</span></div>{/each}</div>{/each}
    <p class="muted">Elimina los que sobren desde la lista; el contenido sigue en el que conserves.</p>
  </section>
{/if}

{#if files.length === 0}
  <div class="card empty"><span class="ico tone-blue"><Icon name="library" size={24} /></span><h2>No hay archivos {q || courseId || kind ? 'con estos filtros' : 'todavía'}</h2>{#if !q && !courseId && !kind}<p class="muted">Descárgalos desde «Mis asignaturas» (botón Descargar en cada recurso) o importa uno propio.</p>{/if}</div>
{/if}

{#each grouped as [title, rows] (title)}
  <section class="grp"><h2>{title} <i>{rows.length}</i></h2>
    <ul>
      {#each rows as f (f.id)}
        <li class:missing={!f.exists}>
          <span class="kbadge k-{f.kind}">{kindGlyph[f.kind]}</span>
          <div class="meta">
            <b>{f.original_name}</b>
            <small class="muted">{f.course_name ?? 'Sin asignatura'}{f.section_position !== null ? ` · ${f.section_position === 0 ? 'General' : 'Tema ' + f.section_position}` : ''}{f.activity_name ? ` · ${f.activity_name}` : ''}</small>
            <small class="muted path">{f.rel_path}{f.sync_state === 'outdated' ? ' · versión anterior' : ''}{!f.exists ? ' · NO ESTÁ EN DISCO' : ''}</small>
          </div>
          <span class="muted nowrap">{fmtSize(f.size_bytes)}<br />{fmtDate(f.downloaded_at)}</span>
          <div class="acts">
            <button class="btn sm primary" disabled={!f.exists} onclick={() => openViewer({ id: f.id, name: f.original_name })}>Ver</button>
            <button class="btn sm" disabled={!f.exists} onclick={() => act(() => fileOpen(f.id))}>Abrir</button>
            <button class="btn sm" disabled={!f.exists} onclick={() => act(() => fileReveal(f.id))}>Carpeta</button>
            <button class="btn sm" onclick={() => (editing = { id: f.id, mode: 'rename', value: f.local_name })}>Renombrar</button>
            <button class="btn sm" onclick={() => (editing = { id: f.id, mode: 'move', value: f.rel_path.split('/').slice(0, -1).join('/') })}>Mover</button>
            {#if confirmDel === f.id}
              <button class="btn sm danger" onclick={() => act(() => fileDelete(f.id), 'Eliminado de tu equipo (Moodle no se toca)')}>Confirmar</button>
              <button class="btn sm" onclick={() => (confirmDel = null)}>No</button>
            {:else}<button class="btn sm" onclick={() => (confirmDel = f.id)}>Eliminar</button>{/if}
          </div>
          {#if editing?.id === f.id}
            <form class="edit" onsubmit={(e) => { e.preventDefault(); submitEdit(); }}>
              <label>{editing.mode === 'rename' ? 'Nuevo nombre' : 'Carpeta destino (dentro de la biblioteca)'}
                <input bind:value={editing.value} list={editing.mode === 'move' ? 'folders' : undefined} />
              </label>
              <button class="btn primary sm">Aceptar</button><button type="button" class="btn sm" onclick={() => (editing = null)}>Cancelar</button>
            </form>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/each}
<datalist id="folders">{#each folders as d}<option value={d}></option>{/each}</datalist>

<style>
  .bar, .bar2 { display: flex; gap: 10px; flex-wrap: wrap; margin: 12px 0; align-items: center; }
  .bar input[type='search'] { flex: 1; min-width: 240px; } .bar2 form { display: flex; gap: 8px; } .bar2 input { min-width: 270px; }
  .grp { margin-top: 24px; animation: rise .5s var(--ease) backwards; } .grp h2 { font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: var(--muted); margin: 0 0 10px 4px; } .grp h2 i { font-style: normal; color: var(--accent); font-weight: 700; margin-left: 6px; }
  ul { list-style: none; margin: 0; padding: 0; background: var(--panel); border: 1px solid var(--line); border-radius: 16px; overflow: hidden; box-shadow: var(--shadow); }
  li { display: grid; grid-template-columns: 52px 1fr auto auto; gap: 14px; align-items: center; padding: 12px 18px; border-top: 1px solid var(--line); transition: background .15s; } li:first-child { border-top: 0; } li:hover { background: var(--panel-2); } li.missing { opacity: .55; }
  .kbadge { width: 46px; height: 34px; border-radius: 9px; display: grid; place-items: center; font-size: 11px; font-weight: 800; letter-spacing: .02em; color: var(--t, var(--c-muted)); background: color-mix(in srgb, var(--t, var(--c-muted)) 15%, transparent); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--t, var(--c-muted)) 25%, transparent); }
  .k-pdf { --t: var(--c-red); } .k-documento { --t: var(--c-blue); } .k-hoja { --t: var(--c-green); } .k-presentación { --t: #fb923c; } .k-comprimido { --t: var(--c-amber); } .k-código { --t: var(--c-violet); } .k-imagen { --t: var(--c-teal); }
  .meta { display: flex; flex-direction: column; gap: 1px; min-width: 0; } .meta b { overflow-wrap: anywhere; font-weight: 600; } .meta small { font-size: 12px; } .path { font-family: 'JetBrains Mono', ui-monospace, monospace; font-size: 11px; overflow-wrap: anywhere; }
  .nowrap { white-space: nowrap; font-size: 12px; text-align: right; } .acts { display: flex; gap: 5px; flex-wrap: wrap; justify-content: flex-end; max-width: 440px; opacity: .6; transition: opacity .2s; } li:hover .acts, li:focus-within .acts { opacity: 1; }
  .edit { grid-column: 1 / -1; display: flex; gap: 8px; align-items: end; flex-wrap: wrap; padding-top: 8px; animation: rise .3s var(--ease) backwards; } .edit label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); flex: 1; min-width: 240px; font-weight: 500; }
  .dups { margin-top: 12px; } .dups .dg { padding: 10px 0; border-top: 1px solid var(--line); } .dups code { font-size: 12px; }
  @media (max-width: 900px) { li { grid-template-columns: 52px 1fr; } .acts { grid-column: 1 / -1; justify-content: flex-start; max-width: none; opacity: 1; } .nowrap { text-align: left; } }
</style>
