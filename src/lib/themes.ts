/** Temas predefinidos. Cada uno fija solo 6 colores base: el resto (bordes, paneles, estados) se deriva en app.css.
 *  `mode` elige la familia de colores de estado (más vivos en oscuro, más profundos en claro). */
export interface Theme {
  id: string; name: string; tagline: string; mode: 'dark' | 'light';
  bg: string; panel: string; text: string; muted: string; accent: string; accentFg: string;
  display?: string; // tipografía de títulos
  /** Tema animado: el acento recorre estas paradas (una cada `secs` segundos, en bucle). `accentFg` debe leerse sobre TODAS. */
  cycle?: { stops: string[]; secs: number };
}

export const themes: Theme[] = [
  { id: 'brasa', name: 'Brasa', tagline: 'Naranja sobre grafito', mode: 'dark', bg: '#121110', panel: '#1b1a17', text: '#f4efe8', muted: '#a39b8f', accent: '#ff6a13', accentFg: '#1a0a00' },
  { id: 'lima', name: 'Lima', tagline: 'Verde ácido sobre negro', mode: 'dark', bg: '#0b0c09', panel: '#141610', text: '#f1f4e8', muted: '#9aa38a', accent: '#c6f432', accentFg: '#0f1500' },
  { id: 'cobalto', name: 'Cobalto', tagline: 'Azul eléctrico sobre marino', mode: 'dark', bg: '#070b17', panel: '#0e1528', text: '#eaf0ff', muted: '#8d9bbd', accent: '#2f64ff', accentFg: '#ffffff' },
  { id: 'fucsia', name: 'Fucsia', tagline: 'Rosa intenso sobre ciruela', mode: 'dark', bg: '#130911', panel: '#1d0f1a', text: '#fbeaf4', muted: '#b394a7', accent: '#ff2d95', accentFg: '#1a0010' },
  { id: 'menta', name: 'Menta', tagline: 'Turquesa sobre verde bosque', mode: 'dark', bg: '#06110e', panel: '#0d1c18', text: '#e6faf3', muted: '#86aa9d', accent: '#19e6b0', accentFg: '#02150f' },
  { id: 'senal', name: 'Señal', tagline: 'Amarillo industrial sobre negro', mode: 'dark', bg: '#0d0c08', panel: '#171610', text: '#f6f1dc', muted: '#a59f86', accent: '#ffc400', accentFg: '#1a1300' },
  { id: 'papel', name: 'Papel', tagline: 'Editorial: crema y bermellón', mode: 'light', bg: '#f1ece0', panel: '#fbf8f0', text: '#17140f', muted: '#6b645a', accent: '#d1330a', accentFg: '#ffffff', display: "'Fraunces', 'Noto Serif Display', Georgia, serif" },
  { id: 'hielo', name: 'Hielo', tagline: 'Claro y técnico, azul puro', mode: 'light', bg: '#eaeff6', panel: '#ffffff', text: '#0c1424', muted: '#5a6781', accent: '#0050ff', accentFg: '#ffffff' },
  { id: 'violeta', name: 'Violeta', tagline: 'Violeta vivo sobre noche', mode: 'dark', bg: '#0e0b17', panel: '#171325', text: '#efeafc', muted: '#a59cc0', accent: '#8456f5', accentFg: '#ffffff' },
  { id: 'coral', name: 'Coral', tagline: 'Coral cálido sobre granate', mode: 'dark', bg: '#150d0d', panel: '#201414', text: '#fbeceb', muted: '#b69a98', accent: '#ff6b6b', accentFg: '#1c0505' },
  { id: 'rubi', name: 'Rubí', tagline: 'Rojo profundo sobre negro', mode: 'dark', bg: '#0f0709', panel: '#190d10', text: '#f7e9ec', muted: '#ad949a', accent: '#e11d48', accentFg: '#ffffff' },
  { id: 'mono', name: 'Mono', tagline: 'Blanco y negro, sin color', mode: 'dark', bg: '#0a0a0a', panel: '#141414', text: '#f2f2f2', muted: '#a0a0a0', accent: '#f2f2f2', accentFg: '#0a0a0a' },
  { id: 'sakura', name: 'Sakura', tagline: 'Claro, rosa cerezo', mode: 'light', bg: '#fbf1f4', panel: '#ffffff', text: '#2a1119', muted: '#79505d', accent: '#c2255c', accentFg: '#ffffff' },
  { id: 'arena', name: 'Arena', tagline: 'Claro, tierra y ámbar', mode: 'light', bg: '#f4ede0', panel: '#fdf9f1', text: '#241a0d', muted: '#6c5a40', accent: '#b45309', accentFg: '#ffffff' },
  { id: 'tinta', name: 'Tinta', tagline: 'Claro, blanco y negro', mode: 'light', bg: '#f6f6f4', panel: '#ffffff', text: '#111111', muted: '#575757', accent: '#111111', accentFg: '#ffffff' },
  // Animados: el acento cambia despacio (transición suave de 2 s entre paradas). Con modo tranquilo se quedan en la primera.
  { id: 'atardecer', name: 'Atardecer', tagline: 'Acento que pasa de naranja a rosa y violeta', mode: 'dark', bg: '#130b12', panel: '#1e121d', text: '#fbeef3', muted: '#b799ac', accent: '#ff7a45', accentFg: '#1a0a05', cycle: { stops: ['#ff7a45', '#ff4d9a', '#b78cff'], secs: 8 } },
  { id: 'marea', name: 'Marea', tagline: 'Acento que fluye entre turquesa, azul y lila', mode: 'dark', bg: '#06100f', panel: '#0d1c1b', text: '#e6f8f6', muted: '#86a9a5', accent: '#19e6b0', accentFg: '#02130f', cycle: { stops: ['#19e6b0', '#5ea0ff', '#b78cff'], secs: 9 } },
  { id: 'espectro', name: 'Espectro', tagline: 'Recorre todos los colores muy despacio', mode: 'dark', bg: '#0b0b0e', panel: '#15151a', text: '#f1f1f6', muted: '#a2a2b2', accent: '#ff5252', accentFg: '#120404', cycle: { stops: ['#ff5252', '#ffb020', '#5be37d', '#35d0ff', '#b78cff'], secs: 12 } },
];

export const radii = { recto: ['3px', '5px'], normal: ['8px', '12px'], redondo: ['12px', '20px'] } as const;
export type RadiusId = keyof typeof radii;

/** Luminancia relativa WCAG de un color #rrggbb. */
export function luminance(hex: string): number {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return 0;
  const n = parseInt(m[1], 16), ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}
export const contrast = (a: string, b: string) => { const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
/** Texto sobre un acento cualquiera: el de mayor contraste real (WCAG), negro o blanco. */
export const readableOn = (hex: string) => (contrast(hex, '#ffffff') >= contrast(hex, '#120c00') ? '#ffffff' : '#120c00');
