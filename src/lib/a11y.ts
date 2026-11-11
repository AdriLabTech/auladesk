/** Acción para diálogos modales: lleva el foco dentro, lo mantiene dentro con Tab y lo devuelve a donde estaba al cerrar. */
const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function trapFocus(node: HTMLElement) {
  const prev = document.activeElement as HTMLElement | null;
  const items = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((e) => e.offsetParent !== null || e === document.activeElement);
  queueMicrotask(() => { if (!node.contains(document.activeElement)) (items()[0] ?? node).focus({ preventScroll: true }); });
  function onKey(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const f = items();
    if (!f.length) { e.preventDefault(); node.focus(); return; }
    const first = f[0], last = f[f.length - 1], a = document.activeElement;
    if (e.shiftKey && (a === first || !node.contains(a))) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && (a === last || !node.contains(a))) { e.preventDefault(); first.focus(); }
  }
  node.addEventListener('keydown', onKey);
  if (!node.hasAttribute('tabindex')) node.tabIndex = -1;
  return { destroy() { node.removeEventListener('keydown', onKey); if (prev && document.contains(prev)) prev.focus({ preventScroll: true }); } };
}
