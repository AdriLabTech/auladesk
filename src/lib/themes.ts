/** Temas predefinidos. Cada uno fija solo 6 colores base: el resto (bordes, paneles, estados) se deriva en app.css.
 *  `mode` elige la familia de colores de estado (más vivos en oscuro, más profundos en claro). */
export interface Theme {
  id: string; name: string; tagline: string; mode: 'dark' | 'light';
  bg: string; panel: string; text: string; muted: string; accent: string; accentFg: string;
  display?: string; // tipografía de títulos
}

export const themes: Theme[] = [
  { id: 'brasa', name: 'Brasa', tagline: 'Naranja sobre grafito', mode: 'dark', bg: '#121110', panel: '#1b1a17', text: '#f4efe8', muted: '#a39b8f', accent: '#ff6a13', accentFg: '#1a0a00' },
  { id: 'lima', name: 'Lima', tagline: 'Verde ácido sobre negro', mode: 'dark', bg: '#0b0c09', panel: '#141610', text: '#f1f4e8', muted: '#9aa38a', accent: '#c6f432', accentFg: '#0f1500' },
  { id: 'cobalto', name: 'Cobalto', tagline: 'Azul eléctrico sobre marino', mode: 'dark', bg: '#070b17', panel: '#0e1528', text: '#eaf0ff', muted: '#8d9bbd', accent: '#2f64ff', accentFg: '#ffffff' },
  { id: 'fucsia', name: 'Fucsia', tagline: 'Rosa intenso sobre ciruela', mode: 'dark', bg: '#130911', panel: '#1d0f1a', text: '#fbeaf4', muted: '#b394a7', accent: '#ff2d95', accentFg: '#ffffff' },
  { id: 'menta', name: 'Menta', tagline: 'Turquesa sobre verde bosque', mode: 'dark', bg: '#06110e', panel: '#0d1c18', text: '#e6faf3', muted: '#86aa9d', accent: '#19e6b0', accentFg: '#02150f' },
  { id: 'senal', name: 'Señal', tagline: 'Amarillo industrial sobre negro', mode: 'dark', bg: '#0d0c08', panel: '#171610', text: '#f6f1dc', muted: '#a59f86', accent: '#ffc400', accentFg: '#1a1300' },
  { id: 'papel', name: 'Papel', tagline: 'Editorial: crema y bermellón', mode: 'light', bg: '#f1ece0', panel: '#fbf8f0', text: '#17140f', muted: '#6b645a', accent: '#e63b0e', accentFg: '#ffffff', display: "'Fraunces', 'Noto Serif Display', Georgia, serif" },
  { id: 'hielo', name: 'Hielo', tagline: 'Claro y técnico, azul puro', mode: 'light', bg: '#eaeff6', panel: '#ffffff', text: '#0c1424', muted: '#5a6781', accent: '#0050ff', accentFg: '#ffffff' },
];

export const radii = { recto: ['3px', '5px'], normal: ['8px', '12px'], redondo: ['12px', '20px'] } as const;
export type RadiusId = keyof typeof radii;

/** Color de texto legible (negro/blanco) sobre un acento cualquiera, por luminancia relativa. */
export function readableOn(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return '#ffffff';
  const n = parseInt(m[1], 16), ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); });
  const L = 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
  return L > 0.42 ? '#120c00' : '#ffffff';
}
