<script lang="ts">
  import { onMount } from 'svelte';
  import { sections, groups, shortcutSections } from './lib/sections';
  import { nav, go } from './lib/nav.svelte';
  import { getSetting, setSetting, moodleStatus, notifUnreadCount, startupSyncStatus } from './lib/api';
  import { fam, refreshFamilias, hasStudents, activeStudent } from './lib/familias.svelte';
  import FamiliasHome from './views/FamiliasHome.svelte';
  import FamiliasGrades from './views/FamiliasGrades.svelte';
  import FamiliasAttendance from './views/FamiliasAttendance.svelte';
  import FamiliasMessages from './views/FamiliasMessages.svelte';
  import FamiliasConnection from './views/FamiliasConnection.svelte';
  import { loadTheme, look } from './lib/theme.svelte';
  import Welcome from './lib/Welcome.svelte';
  import WelcomeAlt from './lib/WelcomeAlt.svelte';
  import Cover from './lib/Cover.svelte';
  import Viewer from './lib/Viewer.svelte';
  import PendingPrompt from './lib/PendingPrompt.svelte';
  import { loadHero } from './lib/herocfg.svelte';
  import { account, refreshAccount, initials } from './lib/account.svelte';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './lib/Icon.svelte';
  import Palette from './lib/Palette.svelte';
  import SyncOverlay from './lib/SyncOverlay.svelte';
  import Fx from './lib/Fx.svelte';
  import { t, loadLang } from './lib/i18n.svelte';
  import { setAudioWanted } from './lib/audio.svelte';
  import { still } from './lib/theme.svelte';
  let hidden = $state(false);
  // El ecualizador solo escucha mientras hay un efecto activo, sin modo tranquilo y con la ventana visible.
  $effect(() => { setAudioWanted(look.fx !== 'none' && !still() && !hidden); });
  import { busy, setExternal } from './lib/busy.svelte';
  import Pending from './views/Pending.svelte';
  import Dashboard from './views/Dashboard.svelte';
  import Settings from './views/Settings.svelte';
  import Connection from './views/Connection.svelte';
  import Courses from './views/Courses.svelte';
  import Library from './views/Library.svelte';
  import Calendar from './views/Calendar.svelte';
  import Notifications from './views/Notifications.svelte';
  import Tasks from './views/Tasks.svelte';
  import Submissions from './views/Submissions.svelte';
  import type { SessionState } from './lib/api';

  // Telón de arranque: tapa el Dashboard hasta que el saludo toma el relevo (o se descarta que vaya a haber saludo).
  let cover = $state(true), themeReady = $state(false);
  let collapsed = $state(false);
  let session = $state<SessionState>('unknown');
  let unread = $state(0);
  const section = $derived(sections.find((s) => s.id === nav.section) ?? sections[0]);
  // Las vistas de datos de Familias solo se ofrecen si hay alumnado sincronizado.
  const visible = $derived(sections.filter((s) => !('needsStudents' in s && s.needsStudents) || hasStudents()));
  const grouped = $derived(groups.map((g) => ({ ...g, items: visible.filter((s) => s.group === g.id) })).filter((g) => g.items.length));
  const ROW = 42, HEAD = 30;
  /** Posición vertical del indicador deslizante: filas + cabeceras de grupo que hay por encima. */
  const pillY = $derived.by(() => {
    let y = 0;
    for (const g of grouped) {
      if (g.label && !collapsed) y += HEAD;
      for (const it of g.items) { if (it.id === nav.section) return y; y += ROW; }
    }
    return 0;
  });
  const ctx = $derived(section.group === 'familias' ? (activeStudent()?.display_name ?? null) : null);

  async function refreshUnread() { try { unread = await notifUnreadCount(); } catch { /* sin BD aún */ } }

  onMount(() => {
    const coverT = setTimeout(() => (cover = false), 3500); // sin sesión que restaurar no habrá saludo: no se retiene la pantalla
    loadLang();
    const vis = () => (hidden = document.hidden); document.addEventListener('visibilitychange', vis);
    loadTheme().then(() => { themeReady = true; if (!look.welcome || look.calm || matchMedia('(prefers-reduced-motion: reduce)').matches) cover = false; }); loadHero();
    (async () => {
      try {
        collapsed = (await getSetting('sidebar_collapsed')) === '1';
        busy.blockSetting = (await getSetting('sync_block')) !== '0';
        session = await moodleStatus();
        refreshUnread(); refreshAccount(); refreshFamilias();
      } catch (e) { console.error(e); }
    })();
    const poll = setInterval(refreshUnread, 30000);
    let un: (() => void) | undefined;
    // Bienvenida: solo al PASAR a sesión activa (la app repite el aviso en cada página de Moodle; eso no cuenta). Con «tranquilo» o sin movimiento no se muestra.
    let wasActive = false;
    listen<SessionState>('moodle-session', async (e) => {
      session = e.payload;
      const active = e.payload === 'active';
      if (active && !wasActive && look.welcome && !look.calm && !matchMedia('(prefers-reduced-motion: reduce)').matches) { await refreshAccount(); nav.welcome = true; clearTimeout(coverT); }
      wasActive = active;
    }).then((u) => (un = u));
    let un2: (() => void) | undefined, un2b: (() => void) | undefined;
    listen('moodle-account', () => refreshAccount()).then((u) => (un2 = u));
    // Sincronización al iniciar (backend): mientras dura se muestra la pantalla de espera; al terminar se recargan los datos de la vista.
    let wasStartup = false;
    const onStartup = (jobs: string[]) => { setExternal(jobs.map((j) => (j === 'Moodle' ? 'Moodle' : 'Familias'))); if (wasStartup && jobs.length === 0) nav.rev += 1; wasStartup = jobs.length > 0; };
    startupSyncStatus().then(onStartup).catch(() => {});
    listen<string[]>('startup-sync', (e) => onStartup(e.payload)).then((u) => (un2b = u));
    const unF: (() => void)[] = [];
    listen('familias-session', () => refreshFamilias()).then((u) => unF.push(u));
    listen('familias-synced', () => { refreshFamilias(); refreshUnread(); }).then((u) => unF.push(u));
    return () => { un2b?.(); document.removeEventListener('visibilitychange', vis); clearTimeout(coverT); clearInterval(poll); un?.(); un2?.(); unF.forEach((u) => u()); };
  });

  function onKey(e: KeyboardEvent) {
    if (busy.blockSetting && busy.jobs.length) return; // el loader bloquea los atajos
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); nav.palette = !nav.palette; return; }
    if (e.ctrlKey && !e.altKey && /^[1-9]$/.test(e.key)) {
      const s = shortcutSections[Number(e.key) - 1];
      if (s) { go(s.id); e.preventDefault(); }
    }
  }
  function toggleSidebar() { collapsed = !collapsed; setSetting('sidebar_collapsed', collapsed ? '1' : '0').catch(console.error); }
  const sessionText = $derived(t(session === 'active' ? 'Conectado' : session === 'expired' ? 'Sesión caducada' : 'Desconectado'));
</script>

<svelte:window onkeydown={onKey} />
<a class="skip" href="#main" onclick={(e) => { e.preventDefault(); document.getElementById('main')?.focus(); }}>{t('Saltar al contenido')}</a>
<div class="sr-only" role="status" aria-live="polite">{t(section.label)}</div>
<div class="shell" class:collapsed>
  <nav class="side" aria-label={t('Secciones')}>
    <div class="brand">
      <span class="logo" aria-hidden="true"><Icon name="cap" size={19} stroke={2} /></span>
      {#if !collapsed}<span class="bname">Aula<b>Desk</b></span>{/if}
      <button class="btn ghost icon collapse" onclick={toggleSidebar} aria-label={collapsed ? t('Expandir barra') : t('Contraer barra')} title={collapsed ? t('Expandir') : t('Contraer')}><Icon name={collapsed ? 'right' : 'left'} size={15} /></button>
    </div>

    <div class="items">
      <span class="pill" style="transform: translateY({pillY}px)" aria-hidden="true"></span>
      {#each grouped as g (g.id)}
        {#if g.label && !collapsed}<span class="ghead">{g.label}</span>{/if}
        {#each g.items as s (s.id)}
          {@const sc = shortcutSections.findIndex((x) => x.id === s.id)}
          <button class="item" class:active={s.id === nav.section} onclick={() => go(s.id)} title={`${g.label ? g.label + ' · ' : ''}${t(s.label)}${sc >= 0 ? ` (Ctrl+${sc + 1})` : ''}`} aria-current={s.id === nav.section ? 'page' : undefined}>
            <Icon name={s.icon} size={18} />
            {#if !collapsed}<span class="lbl">{t(s.label)}</span>{/if}
            {#if s.id === 'notifications' && unread > 0}<span class="count" class:dot={collapsed}>{collapsed ? '' : unread > 99 ? '99+' : unread}</span>{/if}
          </button>
        {/each}
      {/each}
    </div>

    <div class="fxslot"><Fx place="side" /></div>

    <div class="foot">
      <button class="sess" onclick={() => go('moodle')} title={account.name ? `${account.name} · ${sessionText}` : t('Estado de la sesión de Moodle')}>
        {#if account.name}
          <span class="avatar" aria-hidden="true">{initials(account.name)}<i class="pulse-dot" style="--t: {session === 'active' ? 'var(--ok)' : 'var(--warn)'}"></i></span>
          {#if !collapsed}<span class="who"><b>{account.name}</b><small>{sessionText}</small></span>{/if}
        {:else}
          <i class="pulse-dot" style="--t: {session === 'active' ? 'var(--ok)' : 'var(--warn)'}"></i>
          {#if !collapsed}<span class="who"><b>{sessionText}</b><small>Moodle · Junta de Andalucía</small></span>{/if}
        {/if}
      </button>
      <button class="btn ghost icon" onclick={() => go('settings')} aria-label={t('Apariencia y temas')} title={t('Apariencia y temas')}><Icon name="palette" size={16} /></button>
    </div>
  </nav>

  <header class="top" style="position: relative">
    <Fx place="top" />
    <div class="crumbs"><span class="muted">AulaDesk</span><Icon name="right" size={13} />{#if section.group !== 'core'}<span class="muted">{groups.find((g) => g.id === section.group)?.label}{ctx ? ` · ${ctx}` : ''}</span><Icon name="right" size={13} />{/if}<strong>{t(section.label)}</strong></div>
    <button class="search" onclick={() => (nav.palette = true)} aria-label={t('Abrir buscador (Ctrl+K)')}>
      <Icon name="search" size={15} /><span>{t('Buscar secciones, asignaturas, tareas…')}</span><kbd>Ctrl K</kbd>
    </button>
    <button class="bell" class:has={unread > 0} onclick={() => go('notifications')} title={t('Notificaciones sin leer')} aria-label={`${t('Notificaciones')}: ${unread}`}><Icon name="bell" size={17} />{#if unread > 0}<i>{unread > 99 ? '99+' : unread}</i>{/if}</button>
  </header>

  <main class="content" id="main" tabindex="-1" aria-label={t('Contenido principal')}>
    {#key `${nav.section}:${nav.rev}`}
      <div class="view">
        {#if nav.section === 'dashboard'}<Dashboard />
        {:else if nav.section === 'moodle'}<Connection onSession={(s) => (session = s)} />
        {:else if nav.section === 'courses'}<Courses />
        {:else if nav.section === 'tasks'}<Tasks />
        {:else if nav.section === 'submissions'}<Submissions />
        {:else if nav.section === 'calendar'}<Calendar />
        {:else if nav.section === 'notifications'}<Notifications onChange={refreshUnread} />
        {:else if nav.section === 'library'}<Library />
        {:else if nav.section === 'settings'}<Settings />
        {:else if nav.section === 'f_home'}<FamiliasHome />
        {:else if nav.section === 'f_grades'}<FamiliasGrades />
        {:else if nav.section === 'f_attendance'}<FamiliasAttendance />
        {:else if nav.section === 'f_messages'}<FamiliasMessages />
        {:else if nav.section === 'f_conn'}<FamiliasConnection />
        {:else}<Pending title={section.label} phase={section.phase} />{/if}
      </div>
    {/key}
  </main>
</div>
<Palette />
<SyncOverlay />
<Viewer />
<PendingPrompt {session} />
{#if cover}{#if themeReady}<Cover kind={look.resolved} />{:else}<div class="cover" aria-hidden="true"></div>{/if}{/if}
{#if nav.welcome}{#if look.resolved === 'reveal'}<Welcome onDone={() => { nav.welcome = false; cover = false; }} />{:else}<WelcomeAlt kind={look.resolved} onDone={() => { nav.welcome = false; cover = false; }} />{/if}{/if}

<style>
  .fxslot { margin-top: auto; } .fxslot + .foot { margin-top: 8px; } .collapsed .fxslot { display: none; }
  .content:focus { outline: none; }
  .cover { position: fixed; inset: 0; z-index: 150; display: grid; place-items: center; background: var(--bg); }
  .shell { display: grid; grid-template-columns: 248px 1fr; grid-template-rows: 58px 1fr; height: 100%; transition: grid-template-columns .25s var(--ease); }
  .shell.collapsed { grid-template-columns: 68px 1fr; }

  .side { grid-row: 1 / 3; display: flex; flex-direction: column; padding: 14px 12px; gap: 10px; overflow: hidden; background: var(--panel-solid); border-right: 1px solid var(--line); }
  .brand { display: flex; align-items: center; gap: 10px; padding: 2px 4px 12px; min-height: 46px; }
  .logo { width: 32px; height: 32px; border-radius: var(--radius); display: grid; place-items: center; color: var(--accent-fg); background: var(--accent); flex: none; }
  .bname { font-family: var(--font-display); font-size: 17px; font-weight: 500; letter-spacing: -.02em; flex: 1; white-space: nowrap; } .bname b { font-weight: 700; }
  .collapse { padding: 5px; color: var(--muted); } .collapsed .brand { flex-direction: column; gap: 8px; }

  .items { position: relative; display: flex; flex-direction: column; gap: 2px; overflow-y: auto; overflow-x: hidden; }
  .ghead { height: 28px; padding: 10px 14px 0; font-size: 10.5px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; color: var(--faint); box-sizing: border-box; }
  .pill { position: absolute; top: 0; left: 0; right: 0; height: 40px; border-radius: var(--radius); background: var(--accent-soft); transition: transform .35s var(--ease); pointer-events: none; }
  .pill::before { content: ''; position: absolute; left: 0; top: 8px; bottom: 8px; width: 3px; border-radius: 0 3px 3px 0; background: var(--accent); }
  .item { position: relative; height: 40px; display: flex; align-items: center; gap: 12px; background: none; border: 0; border-radius: var(--radius); padding: 0 12px 0 14px; text-align: left; color: var(--muted); white-space: nowrap; transition: color .15s, background .15s; }
  .item:hover { color: var(--text); background: var(--panel-2); } .item.active { color: var(--text); background: none; } .item.active :global(svg) { color: var(--accent); }
  .collapsed .item { justify-content: center; padding: 0; }
  .lbl { flex: 1; font-weight: 550; overflow: hidden; text-overflow: ellipsis; }
  .count { font-size: 11px; font-weight: 800; padding: 1px 7px; border-radius: 99px; background: var(--accent); color: var(--accent-fg); }
  .count.dot { position: absolute; top: 8px; right: 12px; width: 9px; height: 9px; padding: 0; }

  .foot { margin-top: auto; display: flex; align-items: center; gap: 6px; padding-top: 12px; border-top: 1px solid var(--line); } .collapsed .foot { flex-direction: column; }
  .sess { flex: 1; display: flex; align-items: center; gap: 11px; background: var(--panel-2); border: 1px solid var(--line); border-radius: var(--radius); padding: 8px 11px; text-align: left; min-width: 0; transition: border-color .15s; } .sess:hover { border-color: var(--accent-line); }
  .who { display: flex; flex-direction: column; min-width: 0; line-height: 1.25; } .sess b { font-size: 12.5px; font-weight: 650; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .avatar { position: relative; width: 30px; height: 30px; border-radius: var(--radius); display: grid; place-items: center; flex: none; background: var(--accent); color: var(--accent-fg); font: 800 12px/1 var(--font-display); letter-spacing: .02em; }
  .avatar .pulse-dot { position: absolute; right: -3px; bottom: -3px; width: 10px; height: 10px; box-shadow: 0 0 0 2px var(--panel-solid); } .avatar .pulse-dot::after { display: none; } .sess small { font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .top { display: flex; align-items: center; gap: 14px; padding: 0 28px; border-bottom: 1px solid var(--line); background: var(--bg); }
  .crumbs { flex: 1; display: flex; align-items: center; gap: 8px; white-space: nowrap; overflow: hidden; min-width: 0; color: var(--faint); } .crumbs strong { color: var(--text); font-weight: 650; }
  .search { display: flex; align-items: center; gap: 10px; width: 380px; max-width: 42vw; background: var(--panel-2); border: 1px solid var(--line-strong); border-radius: var(--radius); padding: 7px 10px 7px 13px; color: var(--muted); transition: all .15s; }
  .search:hover { border-color: var(--accent-line); color: var(--text); } .search span { flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bell { position: relative; width: 38px; height: 38px; border-radius: var(--radius); display: grid; place-items: center; background: var(--panel-2); border: 1px solid var(--line-strong); color: var(--muted); transition: all .15s; }
  .bell:hover { color: var(--text); border-color: var(--accent-line); } .bell.has { color: var(--accent); }
  .bell.has :global(svg) { animation: ring 3.2s ease-in-out 1s infinite; transform-origin: 50% 10%; }
  .bell i { position: absolute; top: -6px; right: -6px; min-width: 18px; height: 18px; padding: 0 5px; border-radius: 99px; background: var(--accent); color: var(--accent-fg); font: 800 10.5px/18px 'Inter Variable', system-ui; text-align: center; font-style: normal; box-shadow: 0 0 0 2px var(--bg); }
  @keyframes ring { 0%, 8%, 100% { transform: rotate(0); } 2% { transform: rotate(14deg); } 4% { transform: rotate(-12deg); } 6% { transform: rotate(8deg); } }

  .content { overflow: auto; padding: var(--gutter-y, 30px) var(--gutter-x, 36px) 56px; }
  .view { animation: rise .4s var(--ease) backwards; }
  @media (max-width: 1000px) { .search span, .search kbd { display: none; } .search { width: auto; } .content { padding: 20px 18px 40px; } }
</style>
