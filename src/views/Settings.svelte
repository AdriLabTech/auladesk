<script lang="ts">
  import { onMount } from 'svelte';
  import { appInfo, backupDatabase, type AppInfo } from '../lib/api';
  import { themes, radii, type RadiusId } from '../lib/themes';
  import { look, current, setPreset, setAccent, setRadius, setCalm, setWelcome, setDensity, setScale, SCALES, setContrast, setFx, type Fx, setWelcomeKind, resolveWelcome, WELCOME_KINDS, type WelcomeKind } from '../lib/theme.svelte';
  import { nav } from '../lib/nav.svelte';
  import { hero, saveHero, resetHero } from '../lib/herocfg.svelte';
  import DashHero from '../lib/DashHero.svelte';
  import Icon from '../lib/Icon.svelte';
  import { audio } from '../lib/audio.svelte';
  import { t, i18n, setLang, type LangPref } from '../lib/i18n.svelte';
  const groupsOf = $derived([
    { id: 'dark', label: t('Oscuros'), items: themes.filter((x) => x.mode === 'dark' && !x.cycle) },
    { id: 'light', label: t('Claros'), items: themes.filter((x) => x.mode === 'light') },
    { id: 'anim', label: t('Animados (el acento cambia solo)'), items: themes.filter((x) => x.cycle) },
  ]);
  import { busy } from '../lib/busy.svelte';
  import { setSetting } from '../lib/api';
  function setBlock(v: boolean) { busy.blockSetting = v; setSetting('sync_block', v ? '1' : '0').catch(console.error); }

  let info = $state<AppInfo | null>(null);
  let error = $state('');
  let backupMsg = $state('');

  onMount(async () => { try { info = await appInfo(); } catch (e) { error = String(e); } });

  async function backup() {
    backupMsg = '';
    try { backupMsg = `Copia creada: ${await backupDatabase()}`; } catch (e) { error = String(e); }
  }
</script>

<h1>{t('Configuración')}</h1>
{#if error}<p class="card err" role="alert">{error}</p>{/if}

<div class="cfg">

<section class="card">
  <h2>{t('Tema')}</h2>
  <p class="muted">{t('Paletas predefinidas: oscuras, claras y animadas. Cada una cambia colores, contraste y, en «Papel», también la tipografía de los títulos.')}</p>
  {#each groupsOf as g (g.id)}
    <h3 class="gh">{g.label}</h3>
    <div class="themes" role="radiogroup" aria-label={g.label}>
      {#each g.items as th (th.id)}
        <button class="th" class:on={look.id === th.id} role="radio" aria-checked={look.id === th.id} onclick={() => setPreset(th.id)}>
          <span class="sw" style="background: {th.bg}; border-color: {th.text}22">
            <i class="sw-side" style="background: {th.panel}; border-color: {th.text}1a"></i>
            <i class="sw-hero" class:cyc={!!th.cycle} style="background: {th.accent}; --s1: {th.cycle?.stops[0]}; --s2: {th.cycle?.stops[Math.floor(th.cycle.stops.length / 2)]}; --s3: {th.cycle?.stops[th.cycle.stops.length - 1]}"></i>
            <i class="sw-line" style="background: {th.text}"></i><i class="sw-line s" style="background: {th.muted}"></i>
            <i class="sw-chip" style="background: {th.panel}; border-color: {th.text}22"></i>
          </span>
          <b>{t(th.name)}</b><small class="muted">{t(th.tagline)}</small>
          {#if look.id === th.id}<span class="tick"><Icon name="check" size={13} stroke={3} /></span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</section>

<section class="card">
  <h2>{t('Cartel de bienvenida del Dashboard')}</h2>
  <p class="muted">{t('Vista previa en vivo con datos de ejemplo. Los textos admiten')} <code>{'{nombre}'}</code>, <code>{'{tareas}'}</code> {t('y')} <code>{'{vencidas}'}</code>.</p>
  <div class="hprev"><DashHero todo={4} overdue={1} topName="Entrega del proyecto" topReason="vence en 2 d" pct={65} lastSyncAt={Math.floor(Date.now() / 1000) - 600} preview /></div>
  <div class="hgrid">
    <label class="f">{t('Título (vacío = saludo automático según la hora)')}<input maxlength="80" placeholder={t('Ej.: ¡A por ello, {nombre}!')} bind:value={hero.title} oninput={saveHero} /></label>
    <label class="f">{t('Subtítulo (vacío = resumen automático)')}<input maxlength="200" placeholder={t('Ej.: Hoy tienes {tareas} cosas por hacer')} bind:value={hero.subtitle} oninput={saveHero} /></label>
  </div>
  <div class="hsw">
    <label class="chk2"><input type="checkbox" bind:checked={hero.date} onchange={saveHero} /> Fecha</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.account} onchange={saveHero} /> {t('Cuenta de Moodle')}</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.summary} onchange={saveHero} /> Resumen automático</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.ring} onchange={saveHero} /> Anillo de progreso</label>
    <label class="chk2"><input type="checkbox" bind:checked={hero.actions} onchange={saveHero} /> Botones y estado</label>
  </div>
  <div class="opt"><div><b>Color del cartel</b><small class="muted">«Acento» usa el color del tema a lo grande; «Neutro» usa el fondo de panel.</small></div>
    <div class="seg" role="group" aria-label="Color del cartel"><button class:on={hero.tone === 'accent'} onclick={() => { hero.tone = 'accent'; saveHero(); }}>Acento</button><button class:on={hero.tone === 'panel'} onclick={() => { hero.tone = 'panel'; saveHero(); }}>Neutro</button></div></div>
  <button class="btn sm" onclick={resetHero}>{t('Restablecer cartel')}</button>
</section>

<section class="card">
  <h2>{t('Idioma y accesibilidad')}</h2>
  <div class="opt">
    <div><b>{t('Idioma')}</b><small class="muted">{t('Idioma de la interfaz. Los mensajes técnicos que genera la app (errores de Moodle, del portal…) pueden seguir en español.')}</small></div>
    <div class="seg" role="group" aria-label={t('Idioma')}>
      {#each [['auto', t('Automático (según el sistema)')], ['es', 'Español'], ['en', 'English']] as [id, n]}<button class:on={i18n.pref === id} aria-pressed={i18n.pref === id} onclick={() => setLang(id as LangPref)}>{n}</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Alto contraste')}</b><small class="muted">{t('Texto secundario más claro, bordes marcados y foco más grueso. Se activa solo si tu sistema lo pide.')}</small></div>
    <label class="ctl"><input type="checkbox" checked={look.contrast} onchange={(e) => setContrast(e.currentTarget.checked)} /> {t('Activado')}</label>
  </div>
  <div class="opt">
    <div><b>{t('Tamaño de la interfaz')}</b><small class="muted">{t('Amplía o reduce todo el texto y los controles de AulaDesk.')}</small></div>
    <div class="seg" role="group" aria-label={t('Tamaño de la interfaz')}>
      {#each SCALES as z}<button class:on={look.scale === z} aria-pressed={look.scale === z} onclick={() => setScale(z)}>{Math.round(z * 100)} %</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Modo tranquilo')}</b><small class="muted">{t('Sin animaciones ni transiciones. También se activa solo si tu sistema pide reducir movimiento.')}</small></div>
    <label class="ctl"><input type="checkbox" checked={look.calm} onchange={(e) => setCalm(e.currentTarget.checked)} /> {t('Activado')}</label>
  </div>
</section>

<section class="card">
  <h2>{t('Personalizar')}</h2>
  <div class="opt">
    <div><b>{t('Color de acento propio')}</b><small class="muted">{t('Sustituye el acento del tema. El texto sobre él se ajusta solo para que se lea.')}</small></div>
    <div class="ctl">
      <input type="color" value={look.accent ?? current().accent} oninput={(e) => setAccent(e.currentTarget.value)} aria-label="Color de acento" />
      <code>{(look.accent ?? current().accent).toUpperCase()}</code>
      {#if look.accent}<button class="btn sm" onclick={() => setAccent(null)}>Usar el del tema</button>{/if}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Esquinas')}</b><small class="muted">{t('Rectas, normales o redondeadas en toda la app.')}</small></div>
    <div class="seg" role="group" aria-label="Esquinas">
      {#each Object.keys(radii) as r}<button class:on={look.radius === r} onclick={() => setRadius(r as RadiusId)}>{r[0].toUpperCase() + r.slice(1)}</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Pantalla de bienvenida')}</b><small class="muted">{t('Al iniciar sesión se te da la bienvenida con tu nombre y un resumen; la pantalla de carga del arranque cambia a juego. Se omite en modo tranquilo.')}</small></div>
    <div class="ctl wrapc">
      <div class="seg" role="group" aria-label={t('Pantalla de bienvenida')}>
        {#each WELCOME_KINDS as k}<button class:on={look.welcomeKind === k} aria-pressed={look.welcomeKind === k} onclick={() => setWelcomeKind(k as WelcomeKind)}>{t({ reveal: 'Círculo', type: 'Tipográfica', terminal: 'Terminal', grid: 'Cuadrícula', minimal: 'Minimalista', random: 'Aleatoria' }[k])}</button>{/each}
      </div>
      <button class="btn sm" onclick={() => { resolveWelcome(); nav.welcome = true; }}><Icon name="sparkles" size={14} /> {t('Ver ahora')}</button>
      <label class="ctl"><input type="checkbox" checked={look.welcome} onchange={(e) => setWelcome(e.currentTarget.checked)} /> {t('Activada')}</label>
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Efecto decorativo')}</b><small class="muted">{t('Un ecualizador en la barra lateral o una onda bajo la cabecera que reaccionan al sonido que sale por tus altavoces. El audio solo se mide (16 niveles) y se descarta: no se guarda ni se envía. Se apaga con el modo tranquilo o con la ventana oculta.')}</small>
      {#if look.fx !== 'none'}<small class="fxst" role="status">{audio.live ? t('Escuchando la salida de audio') : audio.error ? t('Sin audio real: se muestra una animación decorativa.') + ' ' + audio.error : t('Iniciando…')}</small>{/if}</div>
    <div class="seg" role="group" aria-label={t('Efecto decorativo')}>
      {#each [['none', t('Ninguno')], ['eq', t('Ecualizador')], ['wave', t('Onda')]] as [id, n]}<button class:on={look.fx === id} aria-pressed={look.fx === id} onclick={() => setFx(id as Fx)}>{n}</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Densidad')}</b><small class="muted">{t('Cuánto espacio hay entre tarjetas y márgenes. «Compacta» enseña más contenido a la vez.')}</small></div>
    <div class="seg" role="group" aria-label="Densidad">
      {#each [['compact', 'Compacta'], ['normal', 'Normal'], ['roomy', 'Amplia']] as [id, n]}<button class:on={look.density === id} onclick={() => setDensity(id as 'compact' | 'normal' | 'roomy')}>{n}</button>{/each}
    </div>
  </div>
  <div class="opt">
    <div><b>{t('Bloquear la app al sincronizar')}</b><small class="muted">{t('Muestra un loader a pantalla completa (con la serpiente para esperar) mientras sincronizas o descargas. Si lo desactivas, la app sigue usable.')}</small></div>
    <label class="ctl"><input type="checkbox" checked={busy.blockSetting} onchange={(e) => setBlock(e.currentTarget.checked)} /> Activado</label>
  </div>
</section>

<section class="card">
  <h2>{t('Almacenamiento y diagnóstico')}</h2>
  {#if info}
    <dl>
      <dt>{t('Versión')}</dt><dd>{info.version}</dd>
      <dt>{t('Datos')}</dt><dd><code>{info.data_dir}</code></dd>
      <dt>{t('Base de datos')}</dt><dd><code>{info.db_path}</code> · {info.schema_tables} tablas</dd>
      <dt>{t('Sesión gráfica')}</dt><dd>{info.session_type ?? 'desconocida'}</dd>
    </dl>
    <button class="btn" onclick={backup}>{t('Crear copia de seguridad')}</button>
    {#if backupMsg}<p class="muted">{backupMsg}</p>{/if}
  {:else}<p class="muted">{t('Cargando…')}</p>{/if}
</section>

</div>

<style>
  .wrapc { flex-wrap: wrap; justify-content: flex-end; }
  .fxst { color: var(--muted); font-size: 12px; }
  .gh { margin: 18px 0 8px; font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: var(--muted); }
  .sw-hero.cyc { animation: swcyc 6s steps(1, end) infinite; } @keyframes swcyc { 0% { background: var(--s1); } 33% { background: var(--s2); } 66% { background: var(--s3); } }
  .cfg { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 520px), 1fr)); gap: 16px; margin-top: 18px; align-items: start; width: 100%; }
  .cfg > section { margin: 0; max-width: none; min-width: 0; overflow-wrap: anywhere; }
  h2, h3 { overflow-wrap: anywhere; }
  /* Temas y cartel necesitan todo el ancho; el resto se reparte en columnas */
  .cfg > section:nth-of-type(1), .cfg > section:nth-of-type(2) { grid-column: 1 / -1; }
  section { margin-bottom: 0; animation: rise .4s var(--ease) backwards; } section:nth-of-type(2) { animation-delay: .06s; } section:nth-of-type(3) { animation-delay: .12s; }
  .themes { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 188px), 1fr)); gap: 12px; margin-top: 14px; }
  .th { position: relative; min-width: 0; overflow-wrap: anywhere; text-align: left; display: flex; flex-direction: column; gap: 2px; padding: 10px 10px 12px; border: 1px solid var(--line-strong); border-radius: var(--radius-lg); background: var(--panel-2); transition: border-color .15s, transform .18s var(--ease); }
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
  .opt > div:first-child { display: flex; flex-direction: column; gap: 2px; flex: 1 1 260px; min-width: 0; max-width: 520px; }
  .opt > :not(:first-child) { max-width: 100%; min-width: 0; }
  /* Grupos de botones: wrap con bordes propios en vez de recortar */
  .seg { display: flex; flex-wrap: wrap; gap: 4px; border: 0; overflow: visible; max-width: 100%; }
  .seg button { border: 1px solid var(--line-strong) !important; border-radius: var(--radius); }
  .seg button.on { border-color: var(--accent) !important; } .opt small { font-size: 12.5px; }
  .ctl { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .ctl code { overflow-wrap: anywhere; }
  .hprev { margin: 12px 0 14px; pointer-events: none; min-width: 0; overflow: hidden; border-radius: var(--radius-lg); } .hgrid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr)); gap: 0 12px; align-items: end; } .hgrid > .f, .hgrid input { min-width: 0; } 
  .hsw { display: flex; gap: 8px 22px; flex-wrap: wrap; margin: 6px 0 4px; } .chk2 { display: inline-flex; align-items: center; gap: 8px; font-size: 13.5px; }
  dl { display: grid; grid-template-columns: minmax(80px, 140px) minmax(0, 1fr); gap: 8px 14px; margin: 12px 0 14px; } dt { color: var(--muted); overflow-wrap: anywhere; } dd { margin: 0; overflow-wrap: anywhere; }
  dd code { white-space: normal; overflow-wrap: anywhere; }
</style>
