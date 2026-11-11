import { test } from 'node:test';
import assert from 'node:assert/strict';
import { newGame, step, turn, ringDir, LEN } from '../src/lib/snake.ts';

const fixed = () => 0;
test('en reposo recorre el borde en sentido horario y no se sale ni choca nunca', () => {
  const g = newGame(11);
  for (let i = 0; i < 500; i++) {
    step(g, fixed);
    assert.equal(g.playing, false); assert.equal(g.snake.length, LEN);
    for (const p of g.snake) assert.ok(p.x === 0 || p.y === 0 || p.x === 10 || p.y === 10, `fuera del borde: ${p.x},${p.y}`);
  }
});
test('las esquinas giran en sentido horario', () => {
  assert.equal(ringDir({ x: 10, y: 0 }, 11), 'down'); assert.equal(ringDir({ x: 10, y: 10 }, 11), 'left');
  assert.equal(ringDir({ x: 0, y: 10 }, 11), 'up'); assert.equal(ringDir({ x: 0, y: 0 }, 11), 'right');
});
test('al dirigirla empieza a jugar y aparece comida dentro', () => {
  const g = newGame(11); turn(g, 'down', fixed);
  assert.equal(g.playing, true); assert.ok(g.food && g.food.x > 0 && g.food.y > 0 && g.food.x < 10 && g.food.y < 10);
  step(g, fixed); assert.equal(g.snake[0].y, 1);
});
test('no gira 180°', () => { const g = newGame(11); turn(g, 'left', fixed); assert.equal(g.playing, false); });
test('come y crece', () => {
  const g = newGame(11); turn(g, 'down', fixed); g.food = { x: g.snake[0].x, y: g.snake[0].y + 1 };
  assert.equal(step(g, fixed), true); assert.equal(g.snake.length, LEN + 1);
});
test('al chocar vuelve al recorrido del loader', () => {
  const g = newGame(11); turn(g, 'down', fixed); for (let i = 0; i < 20; i++) step(g, fixed);
  assert.equal(g.playing, false); assert.equal(g.snake.length, LEN); assert.equal(g.food, null);
});
