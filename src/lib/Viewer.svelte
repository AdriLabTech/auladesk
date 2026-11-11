<script lang="ts">
  import { t as tt } from './i18n.svelte';
  import { trapFocus } from './a11y';
  import { tick } from 'svelte';
  import { viewer, closeViewer, kindOf, type ViewKind } from './viewerstate.svelte';
  import { fileRead, fileOpen, fileReveal } from './api';
  import { fmtSize } from './format';
  import Icon from './Icon.svelte';

  let kind = $state<ViewKind>('none');
  let loading = $state(false); let error = $state('');
  let size = $state(0);
  let pdf: any = null;                       // documento de pdf.js (sin tipos: import dinámico)
  let numPages = $state(0); let page = $state(1);
  let scale = $state(1); let fitScale = 1; let fit = $state(true);
  let firstW = $state(595); let firstH = $state(842); // tamaño de la 1.ª página a escala 1 (marcador de posición)
  let scroller = $state<HTMLElement>();
  let imgUrl = $state(''); let text = $state(''); let rows = $state<string[][]>([]); let truncated = $state(false);
  let token = 0;

  $effect(() => { const f = viewer.file; if (f) load(f.id, f.name); else reset(); });
  function reset() { token++; pdf?.destroy?.(); pdf = null; if (imgUrl) URL.revokeObjectURL(imgUrl); imgUrl = ''; text = ''; rows = []; kind = 'none'; numPages = 0; error = ''; truncated = false; fit = true; }

  async function load(id: number, name: string) {
    reset(); const my = ++token; loading = true; kind = kindOf(name);
    try {
      const buf = await fileRead(id); if (my !== token) return;
      size = buf.byteLength;
      if (kind === 'pdf') {
        const pdfjs = await import('pdfjs-dist/legacy/build/pdf.min.mjs');
        const worker = (await import('pdfjs-dist/legacy/build/pdf.worker.min.mjs?url')).default;
        pdfjs.GlobalWorkerOptions.workerSrc = worker;
        const doc = await pdfjs.getDocument({ data: new Uint8Array(buf), isEvalSupported: false }).promise;
        if (my !== token) { doc.destroy(); return; }
        pdf = doc; numPages = doc.numPages;
        const p1 = await doc.getPage(1), v = p1.getViewport({ scale: 1 }); firstW = v.width; firstH = v.height;
        await tick(); refit();
      } else if (kind === 'image') {
        const ext = (name.split('.').pop() ?? '').toLowerCase();
        imgUrl = URL.createObjectURL(new Blob([buf], { type: ext === 'svg' ? 'image/svg+xml' : `image/${ext === 'jpg' ? 'jpeg' : ext}` }));
      } else if (kind === 'text' || kind === 'csv') {
        const MAX = 2 * 1024 * 1024; truncated = buf.byteLength > MAX;
        let t = new TextDecoder('utf-8').decode(new Uint8Array(buf, 0, Math.min(buf.byteLength, MAX)));
        if (kind === 'csv') rows = parseCsv(t, name.toLowerCase().endsWith('.tsv') ? '\t' : t.split('\n')[0].split(';').length > t.split('\n')[0].split(',').length ? ';' : ',');
        else { if (name.toLowerCase().endsWith('.json')) { try { t = JSON.stringify(JSON.parse(t), null, 2); } catch { /* se muestra tal cual */ } } text = t; }
      }
    } catch (e) { if (my === token) error = String(e); } finally { if (my === token) loading = false; }
  }

  function parseCsv(t: string, sep: string): string[][] {
    const out: string[][] = []; let row: string[] = [], cell = '', q = false;
    for (let i = 0; i < t.length && out.length < 500; i++) {
      const c = t[i];
      if (q) { if (c === '"') { if (t[i + 1] === '"') { cell += '"'; i++; } else q = false; } else cell += c; }
      else if (c === '"') q = true; else if (c === sep) { row.push(cell); cell = ''; }
      else if (c === '\n') { row.push(cell.replace(/\r$/, '')); out.push(row); row = []; cell = ''; } else cell += c;
    }
    if (cell || row.length) { row.push(cell); out.push(row); }
    return out;
  }

  function refit() { if (!scroller || !pdf) return; fitScale = Math.min(1.4, Math.max(0.3, (scroller.clientWidth - 56) / firstW)); if (fit) scale = fitScale; }
  function zoom(f: number) { fit = false; scale = Math.min(4, Math.max(0.3, +(scale * f).toFixed(3))); }
  function onScroll() {
    if (!scroller || kind !== 'pdf') return;
    const mid = scroller.getBoundingClientRect().top + scroller.clientHeight / 3;
    const els = scroller.querySelectorAll<HTMLElement>('.pg');
    for (let i = 0; i < els.length; i++) { if (els[i].getBoundingClientRect().bottom > mid) { page = i + 1; return; } }
  }
  function goTo(n: number) { n = Math.min(numPages, Math.max(1, n)); scroller?.querySelectorAll<HTMLElement>('.pg')[n - 1]?.scrollIntoView({ block: 'start' }); }

  /** Pinta la página solo cuando entra en pantalla y la repinta si cambia el zoom. */
  function lazy(node: HTMLElement, p: { n: number; scale: number }) {
    let params = p, visible = false, drawn = 0, task: any = null;
    const canvas = node.querySelector('canvas') as HTMLCanvasElement;
    async function draw() {
      if (!visible || !pdf || drawn === params.scale) return;
      const mine = params.scale;
      try {
        const pg = await pdf.getPage(params.n), dpr = Math.min(window.devicePixelRatio || 1, 2), vp = pg.getViewport({ scale: mine * dpr });
        canvas.width = vp.width; canvas.height = vp.height; canvas.style.width = `${vp.width / dpr}px`; canvas.style.height = `${vp.height / dpr}px`;
        task?.cancel(); task = pg.render({ canvasContext: canvas.getContext('2d')!, viewport: vp }); await task.promise; drawn = mine;
      } catch { /* cancelado por un zoom nuevo */ }
    }
    const io = new IntersectionObserver((es) => { visible = es[0].isIntersecting; draw(); }, { root: scroller, rootMargin: '700px 0px' }); io.observe(node);
    return { update(np: { n: number; scale: number }) { params = np; draw(); }, destroy() { io.disconnect(); task?.cancel(); } };
  }

  function key(e: KeyboardEvent) {
    if (!viewer.file) return;
    if (e.key === 'Escape') { e.preventDefault(); closeViewer(); }
    else if ((e.key === '+' || e.key === '=') && (kind === 'pdf' || kind === 'image')) { e.preventDefault(); zoom(1.2); }
    else if (e.key === '-' && (kind === 'pdf' || kind === 'image')) { e.preventDefault(); zoom(1 / 1.2); }
  }
  let imgZoom = $state(1);
  $effect(() => { if (viewer.file) imgZoom = 1; });
</script>

<svelte:window onkeydown={key} onresize={refit} />
{#if viewer.file}
  <div class="viewer" role="dialog" use:trapFocus aria-modal="true" aria-label={`Visor: ${viewer.file.name}`}>
    <header>
      <div class="ttl"><Icon name="file" size={18} /><b title={viewer.file.name}>{viewer.file.name}</b><small class="muted">{size ? fmtSize(size) : ''}{kind === 'pdf' && numPages ? ` · ${numPages} pág.` : ''}</small></div>
      {#if kind === 'pdf' && numPages}
        <div class="tools">
          <button class="btn sm icon" onclick={() => goTo(page - 1)} aria-label={tt('Página anterior')}><Icon name="up" size={15} /></button>
          <span class="pgn"><input type="number" min="1" max={numPages} value={page} onchange={(e) => goTo(Number(e.currentTarget.value))} aria-label={tt('Página')} /> / {numPages}</span>
          <button class="btn sm icon" onclick={() => goTo(page + 1)} aria-label={tt('Página siguiente')}><Icon name="down" size={15} /></button>
          <span class="sep"></span>
          <button class="btn sm icon" onclick={() => zoom(1 / 1.2)} aria-label={tt('Alejar')}>−</button>
          <span class="zn">{Math.round(scale * 100)}%</span>
          <button class="btn sm icon" onclick={() => zoom(1.2)} aria-label={tt('Acercar')}>+</button>
          <button class="btn sm" class:on={fit} onclick={() => { fit = true; refit(); }}>{tt('Ajustar')}</button>
        </div>
      {:else if kind === 'image'}
        <div class="tools"><button class="btn sm icon" onclick={() => (imgZoom = Math.max(0.1, imgZoom / 1.25))} aria-label={tt('Alejar')}>−</button><span class="zn">{Math.round(imgZoom * 100)}%</span><button class="btn sm icon" onclick={() => (imgZoom = Math.min(8, imgZoom * 1.25))} aria-label={tt('Acercar')}>+</button><button class="btn sm" onclick={() => (imgZoom = 1)}>100%</button></div>
      {/if}
      <div class="right">
        <button class="btn sm" onclick={() => fileOpen(viewer.file!.id).catch((e) => (error = String(e)))}><Icon name="external" size={14} /> {tt('Abrir con el sistema')}</button>
        <button class="btn sm" onclick={() => fileReveal(viewer.file!.id).catch((e) => (error = String(e)))}>{tt('Carpeta')}</button>
        <button class="btn ghost icon" onclick={closeViewer} aria-label={tt('Cerrar visor')}><Icon name="x" size={18} /></button>
      </div>
    </header>

    <div class="body" bind:this={scroller} onscroll={onScroll} class:pdfbody={kind === 'pdf'}>
      {#if loading}<div class="center muted"><span class="spin"></span>{tt('Cargando…')}</div>
      {:else if error}<div class="center"><p class="msg err">{error}</p></div>
      {:else if kind === 'pdf'}
        <div class="pages">
          {#each Array(numPages) as _, i}
            <div class="pg" style="min-width: {firstW * scale}px; min-height: {firstH * scale}px" use:lazy={{ n: i + 1, scale }}><canvas></canvas><span class="pn">{i + 1}</span></div>
          {/each}
        </div>
      {:else if kind === 'image'}
        <div class="imgwrap"><img src={imgUrl} alt={viewer.file.name} style="transform: scale({imgZoom})" /></div>
      {:else if kind === 'csv'}
        <div class="tablewrap">
          <table><tbody>{#each rows as r, ri}<tr class:hd={ri === 0}>{#each r as c}{#if ri === 0}<th>{c}</th>{:else}<td>{c}</td>{/if}{/each}</tr>{/each}</tbody></table>
          {#if truncated || rows.length >= 500}<p class="muted small">{tt('Se muestran las primeras 500 filas.')}</p>{/if}
        </div>
      {:else if kind === 'text'}
        <pre class="code">{text}</pre>{#if truncated}<p class="muted small pad">{tt('Se muestran los primeros 2 MB.')}</p>{/if}
      {:else}
        <div class="center"><div class="card empty"><span class="ico tone-amber"><Icon name="file" size={24} /></span><h2>{tt('Este formato no se puede ver aquí')}</h2><p class="muted">{tt('El visor integrado admite PDF, imágenes, texto/código y CSV. Documentos de Office o comprimidos se abren con el programa del sistema.')}</p>
          <button class="btn primary" onclick={() => fileOpen(viewer.file!.id)}><Icon name="external" size={15} /> {tt('Abrir con el sistema')}</button></div></div>
      {/if}
    </div>
  </div>
{/if}


<style>
  .viewer { position: fixed; inset: 0; z-index: 150; display: flex; flex-direction: column; background: var(--bg); animation: fade .15s backwards; }
  header { display: flex; align-items: center; gap: 14px; padding: 10px 16px; background: var(--panel-solid); border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .ttl { display: flex; align-items: center; gap: 9px; min-width: 0; flex: 1 1 240px; } .ttl b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .ttl :global(svg) { color: var(--accent); }
  .tools { display: flex; align-items: center; gap: 6px; } .sep { width: 1px; height: 20px; background: var(--line-strong); margin: 0 6px; }
  .pgn { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; color: var(--muted); } .pgn input { width: 56px; padding: 3px 6px; text-align: center; }
  .zn { min-width: 48px; text-align: center; font: 600 12px 'JetBrains Mono', ui-monospace, monospace; color: var(--muted); }
  .btn.on { background: var(--accent-soft); border-color: var(--accent-line); } .right { display: flex; gap: 8px; align-items: center; margin-left: auto; }
  .body { flex: 1; overflow: auto; position: relative; } .pdfbody { background: color-mix(in srgb, var(--bg) 70%, #000); }
  .pages { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 24px 0 60px; }
  .pg { position: relative; background: #fff; box-shadow: 0 2px 14px rgba(0, 0, 0, .45); } .pg canvas { display: block; } .pn { position: absolute; right: 8px; bottom: 6px; font: 600 11px 'JetBrains Mono', ui-monospace, monospace; color: #888; }
  .imgwrap { min-height: 100%; display: grid; place-items: center; padding: 24px; overflow: auto; } .imgwrap img { max-width: 100%; transform-origin: center; transition: transform .12s; background: repeating-conic-gradient(var(--panel-2) 0 25%, transparent 0 50%) 0 / 16px 16px; }
  .code { margin: 0; padding: 18px 22px; font: 13px/1.55 'JetBrains Mono', 'JetBrainsMono Nerd Font', ui-monospace, monospace; white-space: pre-wrap; word-break: break-word; tab-size: 4; } .pad { padding: 0 22px 20px; }
  .tablewrap { padding: 18px 22px; overflow: auto; } table { border-collapse: collapse; font-size: 13px; } th, td { border: 1px solid var(--line-strong); padding: 5px 10px; text-align: left; white-space: nowrap; max-width: 380px; overflow: hidden; text-overflow: ellipsis; } th { background: var(--panel-2); position: sticky; top: 0; }
  .center { min-height: 100%; display: grid; place-content: center; justify-items: center; gap: 10px; padding: 24px; }
  .spin { display: inline-block; width: 16px; height: 16px; border-radius: 50%; border: 2px solid var(--accent); border-right-color: transparent; animation: spin .7s linear infinite; vertical-align: -3px; margin-right: 6px; }
</style>