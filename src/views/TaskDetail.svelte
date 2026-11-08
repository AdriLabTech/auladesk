<script lang="ts">
  import { untrack } from 'svelte';
  import { taskSetFlags, noteSet, downloadActivity, moodleOpen, moodleNavigate, listFiles, type TaskRow, type FileRow } from '../lib/api';
  import { fmtDate, fmtSize } from '../lib/format';
  import { openViewer, kindOf } from '../lib/viewerstate.svelte';
  import Icon from '../lib/Icon.svelte';
  import Versions from './Versions.svelte';

  /** Detalle completo de una tarea (descripción, adjuntos, calificación, prioridad, notas y entrega). Lo usan la lista, el tablero y cada asignatura. */
  let { t, onChange = () => {} }: { t: TaskRow; onChange?: () => void | Promise<void> } = $props();
  let error = $state(''); let notice = $state(''); let busy = $state<number | null>(null);
  let noteDraft = $state(''); let taskFiles = $state<FileRow[]>([]); let showRaw = $state(false);

  // Se reinicia solo al cambiar de tarea (no en cada recarga: no pisar una nota a medio escribir)
  $effect(() => { const id = t.id; untrack(() => { noteDraft = t.note ?? ''; notice = ''; error = ''; showRaw = false; taskFiles = []; loadFiles(id); }); });
  async function loadFiles(id: number) { taskFiles = await listFiles({ activity_id: id }).catch(() => []); }
  async function flag(x: TaskRow, f: { priority?: number; favorite?: boolean; doneLocal?: boolean }) { await taskSetFlags(x.id, f).catch((e) => (error = String(e))); await onChange(); }
  async function saveNote(x: TaskRow) { await noteSet(x.id, noteDraft).catch((e) => (error = String(e))); notice = 'Nota guardada'; await onChange(); }
  async function dl(x: TaskRow) {
    error = ''; notice = ''; busy = x.id;
    try { const r = await downloadActivity(x.id); notice = `${r.stored.length} archivo(s) del enunciado${r.errors.length ? ' · ' + r.errors.join(' | ') : ''}`; await onChange(); await loadFiles(x.id); }
    catch (e) { error = String(e); } finally { busy = null; }
  }
  async function inMoodle(x: TaskRow, edit = false) { try { await moodleOpen(); await moodleNavigate(`mod/assign/view.php?id=${x.moodle_cmid}${edit ? '&action=editsubmission' : ''}`); } catch (e) { error = String(e); } }
  const links = (t: TaskRow): { name: string; url: string; file: boolean }[] => { try { return JSON.parse(t.intro_links_json ?? '[]'); } catch { return []; } };
  const rawRows = (t: TaskRow): { group: string; rows: string[][] }[] => {
    try { const j = JSON.parse(t.raw_rows_json ?? '{}'); return [{ group: 'Estado de la entrega', rows: j.status ?? [] }, { group: 'Calificación', rows: j.grade ?? [] }].filter((g) => g.rows.length); } catch { return []; }
  };
  const submittedFiles = (t: TaskRow): { name: string }[] => { try { return JSON.parse(t.submitted_files_json ?? '[]'); } catch { return []; } };
</script>

  {#if error}<p class="msg err" role="alert">{error}</p>{/if}
  {#if notice}<p class="msg ok" role="status">{notice}</p>{/if}
  <h3 class="sec">Descripción</h3>
  {#if t.intro_text}<div class="intro">{t.intro_text}</div>
  {:else}<p class="muted small">Aún no se ha leído la descripción de esta tarea (sincroniza para traerla) o la tarea no tiene.</p>{/if}
  {#if links(t).length}
    <ul class="links">{#each links(t) as l}<li><Icon name={l.file ? 'file' : 'link'} size={14} /><span>{l.name}</span>{#if !l.file}<code class="url" title={l.url}>{l.url}</code>{:else}<small class="muted">adjunto del profesorado</small>{/if}</li>{/each}</ul>
  {/if}
  {#if taskFiles.length}
    <h3 class="sec">Archivos descargados</h3>
    <ul class="links files">{#each taskFiles as f (f.id)}<li><Icon name="file" size={14} /><span>{f.original_name}</span><small class="muted">{fmtSize(f.size_bytes)}</small>
      <button class="btn sm" disabled={!f.exists} onclick={() => openViewer({ id: f.id, name: f.original_name })}>{kindOf(f.original_name) === 'none' ? 'Abrir' : 'Ver'}</button></li>{/each}</ul>
  {/if}
  {#if t.grade || t.feedback || t.grader}
    <h3 class="sec">Calificación</h3>
    <div class="gradebox">{#if t.grade}<b class="gv">{t.grade}</b>{/if}<div>{#if t.grader}<span class="muted small">Calificado por {t.grader}{t.graded_at ? ` · ${fmtDate(t.graded_at)}` : ''}</span>{/if}{#if t.feedback}<p class="fb">{t.feedback}</p>{/if}</div></div>
  {/if}
  <h3 class="sec">Estado y fechas</h3>
  <dl>
    <dt>Estado en Moodle</dt><dd>{t.submission_status ?? '—'}{t.grading_status ? ` · ${t.grading_status}` : ''}</dd>
    {#if t.time_remaining_text}<dt>Tiempo</dt><dd>{t.time_remaining_text}</dd>{/if}
    <dt>Apertura</dt><dd>{t.opens_at ? fmtDate(t.opens_at) : '—'}</dd>
    <dt>Fecha límite</dt><dd>{t.due_at ? fmtDate(t.due_at) : 'Sin fecha: no se inventa ninguna'}</dd>
    {#if t.last_modified_at}<dt>Última modificación</dt><dd>{fmtDate(t.last_modified_at)}</dd>{/if}
    {#if submittedFiles(t).length}<dt>Enviado</dt><dd>{submittedFiles(t).map((f) => f.name).join(', ')}</dd>{/if}
    {#if t.feedback}<dt>Comentarios</dt><dd>{t.feedback}</dd>{/if}
    <dt>Enunciado local</dt><dd>{t.local_files ? `${t.local_files} archivo(s) descargado(s)` : 'sin descargar'}</dd>
    {#if t.assign_synced_at}<dt>Leído de Moodle</dt><dd>{fmtDate(t.assign_synced_at)}</dd>{/if}
  </dl>
  {#if rawRows(t).length}
    <button class="btn ghost sm rawbtn" onclick={() => (showRaw = !showRaw)} aria-expanded={showRaw}><Icon name={showRaw ? 'up' : 'down'} size={14} /> Todos los datos tal cual los muestra Moodle</button>
    {#if showRaw}<div class="raw">{#each rawRows(t) as g}<h4>{g.group}</h4><dl>{#each g.rows as r}<dt>{r[0]}</dt><dd>{r[1]}</dd>{/each}</dl>{/each}</div>{/if}
  {/if}
  <div class="prio-row"><span class="muted small">Prioridad personal</span>
    <div class="seg" role="group" aria-label="Prioridad personal">{#each ['Ninguna', 'Baja', 'Media', 'Alta'] as l, i}<button class:on={t.priority === i} onclick={() => flag(t, { priority: i })}>{l}</button>{/each}</div>
  </div>
  <div class="acts">
    <button class="btn sm" disabled={busy !== null} onclick={() => dl(t)}>{busy === t.id ? 'Descargando…' : 'Descargar enunciado'}</button>
    <button class="btn sm" onclick={() => inMoodle(t)}>Ver en Moodle</button>
    {#if t.can_submit !== 0 && submittedFiles(t).length}<button class="btn sm" onclick={() => inMoodle(t, true)} title="Abre el formulario oficial de edición de la entrega en Moodle">Modificar entrega en Moodle</button>{/if}
    <button class="btn sm" onclick={() => flag(t, { favorite: !t.favorite })}>{t.favorite ? 'Quitar favorita' : 'Marcar favorita'}</button>
    <button class="btn sm" onclick={() => flag(t, { doneLocal: !t.done_local })}>{t.done_local ? 'Desmarcar «hecha»' : 'Marcar «hecha» (solo local)'}</button>
  </div>
  <label class="note">Nota personal
    <textarea rows="2" bind:value={noteDraft} placeholder="Solo en tu equipo, Moodle no la ve"></textarea>
  </label>
  <button class="btn sm" onclick={() => saveNote(t)}>Guardar nota</button>
  <h3>Mi entrega</h3>
  <Versions activityId={t.id} canPrepare={t.can_submit !== 0} hasSubmission={submittedFiles(t).length > 0} onChange={onChange} />

<style>
  h3 { font-size: 13px; margin: 20px 0 8px; } h3.sec { font-family: var(--font-display); font-size: 12px; text-transform: uppercase; letter-spacing: .09em; color: var(--accent); margin: 18px 0 8px; }
  .intro { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 13.5px; line-height: 1.6; padding: 12px 14px; background: var(--panel-2); border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: var(--radius); max-height: 340px; overflow: auto; }
  .links { list-style: none; margin: 10px 0 0; padding: 0; display: flex; flex-direction: column; gap: 6px; } .links li { display: flex; align-items: center; gap: 9px; font-size: 13px; min-width: 0; } .links li :global(svg) { color: var(--accent); flex: none; }
  .links li > span { overflow-wrap: anywhere; } .url { font-size: 11px; max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); } .files li .btn { margin-left: auto; }
  .gradebox { display: flex; gap: 16px; align-items: flex-start; padding: 12px 14px; border: 1px solid color-mix(in srgb, var(--ok) 40%, transparent); border-radius: var(--radius); background: color-mix(in srgb, var(--ok) 8%, transparent); } .gv { font-family: var(--font-display); font-size: 26px; color: var(--ok); } .fb { margin: 4px 0 0; white-space: pre-wrap; font-size: 13.5px; }
  .rawbtn { margin: 4px 0 0 -8px; } .raw { margin-top: 6px; padding: 4px 14px 10px; background: var(--panel-2); border-radius: var(--radius); } .raw h4 { margin: 10px 0 2px; font-size: 12px; color: var(--muted); }
  dl { display: grid; grid-template-columns: 150px 1fr; gap: 6px 14px; margin: 12px 0; font-size: 13.5px; } dt { color: var(--muted); } dd { margin: 0; overflow-wrap: anywhere; }
  .prio-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin: 12px 0 4px; }
  .acts { display: flex; gap: 8px; flex-wrap: wrap; margin: 10px 0; }
  .note { display: flex; flex-direction: column; gap: 5px; font-size: 12.5px; color: var(--muted); margin: 10px 0 8px; font-weight: 550; }
</style>
