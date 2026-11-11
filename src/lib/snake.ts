/** El loader ES la serpiente: en reposo sus cuadrados recorren el borde de un cuadrado (autopiloto). Al dirigirla con flechas/deslizamiento
 *  sale del borde y juega; si choca, vuelve al recorrido del loader. Sin puntuaciones. Lógica pura (sin DOM) para poder probarla. */
export type Dir = 'up' | 'down' | 'left' | 'right';
export interface Pt { x: number; y: number }
export interface Game { size: number; snake: Pt[]; dir: Dir; next: Dir; food: Pt | null; playing: boolean }

const D: Record<Dir, Pt> = { up: { x: 0, y: -1 }, down: { x: 0, y: 1 }, left: { x: -1, y: 0 }, right: { x: 1, y: 0 } };
const OPP: Record<Dir, Dir> = { up: 'down', down: 'up', left: 'right', right: 'left' };
export const LEN = 8;

/** Serpiente en el borde superior, avanzando a la derecha (sentido horario). */
export function newGame(size = 11): Game {
  const snake = Array.from({ length: LEN }, (_, i) => ({ x: LEN - 1 - i, y: 0 }));
  return { size, snake, dir: 'right', next: 'right', food: null, playing: false };
}
/** Dirección horaria que toca en el borde según dónde esté la cabeza. */
export function ringDir(p: Pt, size: number): Dir {
  const m = size - 1;
  if (p.y === 0 && p.x < m) return 'right';
  if (p.x === m && p.y < m) return 'down';
  if (p.y === m && p.x > 0) return 'left';
  return 'up';
}
export function place(size: number, snake: Pt[], rnd: () => number): Pt {
  const free: Pt[] = [];
  for (let y = 1; y < size - 1; y++) for (let x = 1; x < size - 1; x++) if (!snake.some((s) => s.x === x && s.y === y)) free.push({ x, y });
  return free.length ? free[Math.floor(rnd() * free.length) % free.length] : { x: 1, y: 1 };
}
/** El jugador toma el mando: ignora el giro de 180° y, si estaba en el recorrido, empieza a jugar con comida en el tablero. */
export function turn(g: Game, d: Dir, rnd: () => number = Math.random) {
  if (d === OPP[g.dir]) return;
  g.next = d;
  if (!g.playing) { g.playing = true; g.food = place(g.size, g.snake, rnd); }
}
/** Avanza una casilla. Choque = vuelve al recorrido del loader. Devuelve true si comió. */
export function step(g: Game, rnd: () => number = Math.random): boolean {
  if (!g.playing) { g.next = ringDir(g.snake[0], g.size); }
  g.dir = g.next;
  const h = g.snake[0], d = D[g.dir], n = { x: h.x + d.x, y: h.y + d.y };
  const eats = !!g.food && n.x === g.food.x && n.y === g.food.y;
  const body = eats ? g.snake : g.snake.slice(0, -1);
  if (n.x < 0 || n.y < 0 || n.x >= g.size || n.y >= g.size || body.some((s) => s.x === n.x && s.y === n.y)) { Object.assign(g, newGame(g.size)); return false; }
  g.snake = [n, ...body];
  if (eats) g.food = place(g.size, g.snake, rnd);
  return eats;
}
/** Milisegundos por paso. */
export const TICK = 110;
