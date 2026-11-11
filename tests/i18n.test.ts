import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { en } from '../src/lib/en.ts';
import { errors } from '../src/lib/en_errors.ts';

const walk = (d: string): string[] => readdirSync(d).flatMap((f) => { const p = join(d, f); return statSync(p).isDirectory() ? walk(p) : /\.(svelte|ts)$/.test(f) ? [p] : []; });
const files = walk('src').filter((f) => !f.endsWith('en.ts') && !f.endsWith('en_errors.ts') && !f.endsWith('i18n.svelte.ts'));

test('toda cadena pasada a t()/tt()/tr() tiene traducción al inglés', () => {
  const missing = new Set<string>();
  for (const f of files) {
    const src = readFileSync(f, 'utf8');
    for (const m of src.matchAll(/\b(?:tt|tr|t)\(\s*'((?:[^'\\]|\\.)+)'/g)) {
      const k = m[1].replace(/\\'/g, "'");
      if (!(k in en)) missing.add(`${f.replace('src/', '')}: ${k}`);
    }
  }
  assert.deepEqual([...missing], [], `sin traducción:\n${[...missing].join('\n')}`);
});
test('las traducciones conservan las variables {x}', () => {
  for (const [k, v] of Object.entries(en)) {
    const vars = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join();
    assert.equal(vars(k), vars(v), `variables distintas en «${k}»`);
  }
});
test('errores: mismos huecos {} en ambos idiomas', () => { for (const [es, e] of errors) assert.equal(es.split('{}').length, e.split('{}').length, es); });
