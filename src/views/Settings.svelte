<script lang="ts">
  import { onMount } from 'svelte';
  import { appInfo, backupDatabase, type AppInfo } from '../lib/api';
  import { themes, radii, type RadiusId } from '../lib/themes';
  import { look, current, setPreset, setAccent, setRadius, setCalm, setWelcome } from '../lib/theme.svelte';
  import { nav } from '../lib/nav.svelte';
  import { hero, saveHero, resetHero } from '../lib/herocfg.svelte';
  import DashHero from '../lib/DashHero.svelte';
  import Icon from '../lib/Icon.svelte';

  let info = $state<AppInfo | null>(null);
  let error = $state('');
  let backupMsg = $state('');

  onMount(async () => { try { info = await appInfo(); } catch (e) { error = String(e); } });

  async function backup() {
    backupMsg = '';
    try { backupMsg = `Copia creada: ${await backupDatabase()}`; } catch (e) { error = String(e); }
  }
</script>

<h1>Configuración</h1>
{#if error}<p class="card err" role="alert">{error}</p>{/if}

<section class="card">
  <h2>Tema</h2>
  <p class="muted">Ocho paletas predefinidas. Cada una cambia colores, contraste y, en «Papel», también la tipografía de los títulos.</p>
  <div class="themes" role="radiogroup" aria-label="Tema">
    {#each themes as t (t.id)}
      <button class="th" class:on={look.id === t.id} role="radio" aria-checked={look.id === t.id} onclick={() => setPreset(t.id)}>
        <span class="sw" style="background: {t.bg}; border-color: {t.text}22">
          <i class="sw-side" style="background: {t.panel}; border-color: {t.text}1a"></i>
          <i class="sw-hero" style="background: {t.accent}"></i>
          <i class="sw-line" style="background: {t.text}"></i><i class="sw-line s" style="background: {t.muted}"></i>
          <i class="sw-chip" style="background: {t.panel}; border-color: {t.text}22"></i>
        </span>
        <b>{t.name}</b><small class="muted">{t.tagline}</small>
        {#if look.id === t.id}<span class="tick"><Icon name="check" size={13} stroke={3} /></span>{/if}
      </button>
    {/each}
  </div>
</section>

<section class="card">
  <h2>Cartel de bienvenida del Dashboard</h2>
  <p class="muted">Vista previa en vivo con datos de ejemplo. Los textos admiten <code>{'{nombre}'}</code>, <code>{'{tareas}'}</code> y <code>{'{vencidas}'}</code>.</p>
  <div class="hprev"><DashHero todo={4} overdue={1} topName="Entrega del proyecto" topReason="vence en 2 d" pct={65} lastSyncAt={Math.floor(Date.now() / 1000) - 600} preview /></div>
  <div class="hgrid">
    <label class="f">Título (vacío = saludo automático según la hora)<input maxlength="80" placeholder="Ej.: ¡A por ello, {'{nombre}'}!" bind:value={hero.title} oninput={saveHero} /></label>
    <label class="f">Subtítulo (vacío = resumen automático)<input maxlength="200" placeholder="Ej.: Hoy tienes {'{tareas}'} cosas por hacer" bind:value={hero.subtitle} oninput={saveHero} /></label>
  </div>
  <div class="hsw">
    <label class="chk2"><input type="checkbox" bind:checked={hero.date} onchange={saveHero} /> Fecha</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.account} onchange={saveHero} /> Cuenta de Moodle</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.summary} onchange={saveHero} /> Resumen automático</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.ring} onchange={saveHero} /> Anillo de progreso</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.actions} onchange={saveHero} /> Botones y estado</label>
  </div>
  <div class="opt"><div><b>Color del cartel</b><small class="muted">«Acento» usa el color del tema a lo grande; «Neutro» usa el fondo de panel.</small></div>
    <div class="seg" role="group" aria-label="Color del cartel"><button class:on={hero.tone === 'accent'} onclick={() => { hero.tone = 'accent'; saveHero(); }}>Acento</button><button class:on={hero.tone === 'panel'} onclick={() => { hero.tone = 'panel'; saveHero(); }}>Neutro</button></div></div>
  <button class="btn sm" onclick={resetHero}>Restablecer cartel</button>
</section>

<section class="card">
  <h2>Personalizar</h2>
  <div class="opt">
    <div><b>Color de acento propio</b><small class="muted">Sustituye el acento del tema. El texto sobre él se ajusta solo para que se lea.</small></div>
    <div class="ctl">
      <input type="color" value={look.accent ?? current().accent} oninput={(e) => setAccent(e.currentTarget.value)} aria-label="Color de acento" />
      <code>{(look.accent ?? current().accent).toUpperCase()}</code>
      {#if look.accent}<button class="btn sm" onclick={() => setAccent(null)}>Usar el del tema</button>{/if}
    </div>
  </div>
  <div class="opt">
    <div><b>Esquinas</b><small class="muted">Rectas, normales o redondeadas en toda la app.</small></div>
    <div class="seg" role="group" aria-label="Esquinas">
      {#each Object.keys(radii) as r}<button class:on={look.radius === r} onclick={() => setRadius(r as RadiusId)}>{r[0].toUpperCase() + r.slice(1)}</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>Animación de bienvenida</b><small class="muted">Al iniciar sesión se te da la bienvenida con tu nombre y un resumen. Se omite en modo tranquilo.</small></div>
    <div class="ctl">
      <button class="btn sm" onclick={() => (nav.welcome = true)}><Icon name="sparkles" size={14} /> Ver ahora</button>
      <label class="ctl"><input type="checkbox" checked={look.welcome} onchange={(e) => setWelcome(e.currentTarget.checked)} /> Activada</label>
    </div>
  </div>
  <div class="opt">
    <div><b>Modo tranquilo</b><small class="muted">Sin animaciones ni transiciones. También se activa solo si tu sistema pide reducir movimiento.</small></div>
    <label class="ctl"><input type="checkbox" checked={look.calm} onchange={(e) => setCalm(e.currentTarget.checked)} /> Activado</label>
  </div>
</section>

<section class="card">
  <h2>Almacenamiento y diagnóstico</h2>
  {#if info}
    <dl>
      <dt>Versión</dt><dd>{info.version}</dd>
      <dt>Datos</dt><dd><code>{info.data_dir}</code></dd>
      <dt>Base de datos</dt><dd><code>{info.db_path}</code> · {info.schema_tables} tablas</dd>
      <dt>Sesión gráfica</dt><dd>{info.session_type ?? 'desconocida'}</dd>
    </dl>
    <button class="btn" onclick={backup}>Crear copia de seguridad</button>
    {#if backupMsg}<p class="muted">{backupMsg}</p>{/if}
  {:else}<p class="muted">Cargando…</p>{/if}
</section>

<style>
  section { margin-bottom: 16px; max-width: 880px; animation: rise .4s var(--ease) backwards; } section:nth-of-type(2) { animation-delay: .06s; } section:nth-of-type(3) { animation-delay: .12s; }
  .themes { display: grid; grid-template-columns: repeat(auto-fill, minmax(188px, 1fr)); gap: 12px; margin-top: 14px; }
  .th { position: relative; text-align: left; display: flex; flex-direction: column; gap: 2px; padding: 10px 10px 12px; border: 1px solid var(--line-strong); border-radius: var(--radius-lg); background: var(--panel-2); transition: border-color .15s, transform .18s var(--ease); }
  .th:hover { transform: translateY(-2px); border-color: var(--accent-line); } .th.on { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent); }
  .th b { margin-top: 8px; font-family: var(--font-display); font-size: 15px; } .th small { font-size: 12px; }
  .sw { position: relative; display: block; height: 82px; border-radius: var(--radius); border: 1px solid; overflow: hidden; }
  .sw i { position: absolute; display: block; border: 1px solid transparent; font-style: normal; }
  .sw-side { left: 0; top: 0; bottom: 0; width: 22%; border-width: 0 1px 0 0 !important; }
  .sw-hero { left: 29%; top: 10px; right: 10px; height: 26px; border-radius: 4px; }
  .sw-line { left: 29%; top: 46px; width: 46%; height: 5px; border-radius: 3px; opacity: .85; } .sw-line.s { top: 57px; width: 30%; opacity: .45; }
  .sw-chip { left: 29%; bottom: 7px; width: 22%; height: 9px; border-radius: 3px; }
  .tick { position: absolute; top: 16px; right: 16px; width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; background: var(--accent); color: var(--accent-fg); }
  .opt { display: flex; justify-content: space-between; align-items: center; gap: 20px; padding: 14px 0; border-top: 1px solid var(--line); flex-wrap: wrap; } .opt:first-of-type { margin-top: 10px; }
  .opt > div:first-child { display: flex; flex-direction: column; gap: 2px; max-width: 520px; } .opt small { font-size: 12.5px; }
  .ctl { display: flex; align-items: center; gap: 10px; }
  .hprev { margin: 12px 0 14px; pointer-events: none; transform-origin: top left; } .hgrid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; } @media (max-width: 800px) { .hgrid { grid-template-columns: 1fr; } }
  .hsw { display: flex; gap: 8px 22px; flex-wrap: wrap; margin: 6px 0 4px; } .chk2 { display: inline-flex; align-items: center; gap: 8px; font-size: 13.5px; }
  dl { display: grid; grid-template-columns: 140px 1fr; gap: 8px 14px; margin: 12px 0 14px; } dt { color: var(--muted); } dd { margin: 0; word-break: break-all; }
</style>
