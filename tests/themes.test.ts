import { test } from 'node:test';
import assert from 'node:assert/strict';
import { themes, contrast, readableOn } from '../src/lib/themes.ts';

test('ids únicos', () => assert.equal(new Set(themes.map((t) => t.id)).size, themes.length));
for (const t of themes) {
  test(`${t.id}: contraste legible (WCAG AA)`, () => {
    assert.ok(contrast(t.text, t.bg) >= 7, `texto/fondo ${contrast(t.text, t.bg).toFixed(1)}`);
    assert.ok(contrast(t.muted, t.bg) >= 4.5, `secundario/fondo ${contrast(t.muted, t.bg).toFixed(1)}`);
    assert.ok(contrast(t.muted, t.panel) >= 4.5, `secundario/panel ${contrast(t.muted, t.panel).toFixed(1)}`);
    for (const a of t.cycle ? t.cycle.stops : [t.accent]) {
      assert.ok(contrast(t.accentFg, a) >= 4.5, `texto sobre acento ${a}: ${contrast(t.accentFg, a).toFixed(1)}`);
      assert.ok(contrast(a, t.bg) >= 3, `acento ${a} sobre fondo (componente de interfaz) ${contrast(a, t.bg).toFixed(1)}`);
    }
  });
}
test('readableOn elige el mayor contraste', () => { assert.equal(readableOn('#ff2d95'), '#120c00'); assert.equal(readableOn('#0a0a0a'), '#ffffff'); });
