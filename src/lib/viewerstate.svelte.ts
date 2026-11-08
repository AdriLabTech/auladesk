/** Archivo abierto en el visor integrado (null = cerrado). */
export const viewer = $state<{ file: { id: number; name: string } | null }>({ file: null });
export const openViewer = (file: { id: number; name: string }) => { viewer.file = file; };
export const closeViewer = () => { viewer.file = null; };

const IMG = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'avif'];
const TEXT = ['txt', 'md', 'markdown', 'log', 'csv', 'tsv', 'json', 'xml', 'html', 'htm', 'css', 'js', 'mjs', 'ts', 'tsx', 'jsx', 'java', 'cs', 'py', 'rs', 'c', 'cpp', 'h', 'hpp', 'sql', 'sh', 'bash', 'yml', 'yaml', 'toml', 'ini', 'properties', 'gradle', 'kt', 'go', 'php', 'rb', 'svelte', 'vue', 'xaml', 'csproj', 'sln', 'bat', 'ps1', 'tex', 'r', 'swift', 'dart', 'lua'];
export type ViewKind = 'pdf' | 'image' | 'csv' | 'text' | 'none';
export function kindOf(name: string): ViewKind {
  const ext = (name.split('.').pop() ?? '').toLowerCase();
  if (ext === 'pdf') return 'pdf';
  if (IMG.includes(ext)) return 'image';
  if (ext === 'csv' || ext === 'tsv') return 'csv';
  if (TEXT.includes(ext)) return 'text';
  return 'none';
}
