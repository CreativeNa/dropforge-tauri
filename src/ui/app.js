const api = window.dropforge ?? null;

// в приложении только левый клик: убираем контекстное меню WebView2
let resizeTimer = null;
let searchTimer = null;
window.addEventListener('resize', () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    state.page = 0;
    render();
  }, 250);
});

document.addEventListener('contextmenu', (event) => {
  event.preventDefault();
}, { capture: true });

// пока тянем ползунок, не перерисовываем интерфейс: иначе DOM-элемент
// пересоздаётся и перетаскивание обрывается
let sliderDragging = false;
let sliderPendingRender = false;
const isSlider = (target) => target && target.matches && target.matches('input[type="range"]');
document.addEventListener('pointerdown', (event) => {
  if (isSlider(event.target)) sliderDragging = true;
}, true);
const finishSlider = () => {
  if (!sliderDragging) return;
  sliderDragging = false;
  if (sliderPendingRender) {
    sliderPendingRender = false;
    render();
  }
};
document.addEventListener('pointerup', finishSlider, true);
document.addEventListener('pointercancel', finishSlider, true);
window.addEventListener('blur', finishSlider);



/* ------------------------------------------------------------------ icons */

const ICONS = {
  logo: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v6"/><path d="M5 8h14l-1.5 12.5a2 2 0 0 1-2 1.5H8.5a2 2 0 0 1-2-1.5L5 8Z"/><path d="M9 12h6"/></svg>',
  drops: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8.5 12 4l8 4.5v7L12 20l-8-4.5v-7Z"/><path d="M4 8.5 12 13l8-4.5M12 13v7"/></svg>',
  log: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4h11l3 3v13H5z"/><path d="M8 9h8M8 13h8M8 17h5"/></svg>',
  gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2 2 2 0 1 1-4 0 1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3 15a2 2 0 1 1 0-4 1.7 1.7 0 0 0 1.4-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10 4a2 2 0 1 1 4 0 1.7 1.7 0 0 0 2.9 1.4l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1A1.7 1.7 0 0 0 21 11a2 2 0 1 1 0 4Z"/></svg>',
  search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>',
  refresh: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.6-6.4"/><path d="M21 4v5h-5"/></svg>',
  play: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.5v13l11-6.5-11-6.5Z"/></svg>',
  stop: '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="7" y="7" width="10" height="10" rx="1.6"/></svg>',
  plus: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
  check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="m5 13 4.5 4.5L19 7"/></svg>',
  close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18"/></svg>',
  logout: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M15 4h4v16h-4"/><path d="M11 8l4 4-4 4M15 12H3"/></svg>',
  shield: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l7 3v6c0 4.4-3 7.5-7 9-4-1.5-7-4.6-7-9V6l7-3Z"/><path d="m9 12 2 2 4-4"/></svg>',
  bolt: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2 4 14h6l-1 8 9-12h-6l1-8Z"/></svg>',
  flame: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3s5 4.5 5 9a5 5 0 0 1-10 0c0-2 1-3.5 2-4.5 0 2 1 3 2 3 0-2 1-5 1-7.5Z"/></svg>',
  gift: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 10h16v10H4z"/><path d="M2 6h20v4H2zM12 6v14"/><path d="M12 6S9.5 2 7.5 3.4C5.8 4.6 7 6 12 6Zm0 0s2.5-4 4.5-2.6C18.2 4.6 17 6 12 6Z"/></svg>',
  folder: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7h6l2 2h10v10H3z"/></svg>',
  star: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="m12 3.7 2.5 5.1 5.6.8-4 4 1 5.6-5.1-2.7-5.1 2.7 1-5.6-4-4 5.6-.8Z"/></svg>',
  starOn: '<svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="m12 3.7 2.5 5.1 5.6.8-4 4 1 5.6-5.1-2.7-5.1 2.7 1-5.6-4-4 5.6-.8Z"/></svg>',
  pause: '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="7" y="5" width="4" height="14" rx="1.4"/><rect x="13" y="5" width="4" height="14" rx="1.4"/></svg>',
  trash: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16"/><path d="M9 7V5.5A1.5 1.5 0 0 1 10.5 4h3A1.5 1.5 0 0 1 15 5.5V7"/><path d="M6.5 7 7.4 19a1.5 1.5 0 0 0 1.5 1.4h6.2a1.5 1.5 0 0 0 1.5-1.4L17.5 7"/><path d="M10.5 11v5.5M13.5 11v5.5"/></svg>',
};

/* ------------------------------------------------------------------- state */

let pendingSheetGame = null;

const state = {
  snapshot: null,
  view: 'drops',
  streakMenu: false,
  streakChannel: null,
  streakClosing: false,
  streakAnimating: false,
  query: '',
  dropsTab: 'global',
  page: 0,
  sheetGame: null,
  busy: new Set(),
  device: null,
  error: null,
  demo: false,
  loading: true,
  update: null,
  diagnostics: null,
  scrollTop: 0,
  appVersion: null,
};

/* ----------------------------------------------------------------- helpers */

const esc = (value) =>
  String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');

const clamp01 = (value) => Math.max(0, Math.min(1, Number(value) || 0));

function boxArt(game) {
  if (game?.boxArtUrl) return game.boxArtUrl;
  if (/gtavi/i.test(game?.name ?? '')) return 'https://static-cdn.jtvnw.net/ttv-boxart/1867279146_IGDB-285x380.jpg';
  return null;
}

function percent(value) {
  return `${Math.round(clamp01(value) * 100)}%`;
}

function formatLeft(iso) {
  if (!iso) return 'без срока';
  const diff = new Date(iso).getTime() - Date.now();
  if (Number.isNaN(diff)) return 'без срока';
  if (diff <= 0) return 'завершено';
  const totalMinutes = Math.floor(diff / 60000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return `${days} д ${hours} ч`;
  if (hours > 0) return `${hours} ч ${minutes} мин`;
  return `${Math.max(1, minutes)} мин`;
}

function formatDate(iso) {
  if (!iso) return '—';
  return new Intl.DateTimeFormat('ru-RU', { day: 'numeric', month: 'short' }).format(new Date(iso));
}

function formatMinutes(minutes) {
  const total = Math.max(0, Math.round(minutes));
  const hours = Math.floor(total / 60);
  const rest = total % 60;
  if (hours && rest) return `${hours} ч ${rest} м`;
  if (hours) return `${hours} ч`;
  return `${rest} м`;
}

function initials(name) {
  return String(name ?? '?')
    .split(/\s+/)
    .slice(0, 2)
    .map((word) => word[0] ?? '')
    .join('')
    .toUpperCase();
}

function toast(message, kind = 'info') {
  const host = document.getElementById('toasts');
  if (!host) return;
  const node = document.createElement('div');
  node.className = `toast ${kind === 'info' ? '' : kind}`;
  node.textContent = message;
  host.append(node);
  setTimeout(() => node.remove(), 5200);
}

let autoRefreshRunning = false;
let refreshStartedAt = 0;

/**
 * Первый чистый запуск или вход: каталога ещё нет — тянем его сами и держим
 * спиннер, пока бэкенд не пришлёт refresh.active = false.
 */
function maybeAutoRefresh() {
  if (autoRefreshRunning) return;
  if (!state.snapshot?.auth?.loggedIn) return;
  if ((state.snapshot.stats?.campaigns ?? 0) > 0) return;
  autoRefreshRunning = true;
  refreshStartedAt = Date.now();
  setBusy('refresh', true);
  render();
  api
    .refresh()
    .then((snapshot) => {
      if (snapshot) applySnapshot(snapshot);
    })
    .catch(() => {})
    .finally(() => {
      autoRefreshRunning = false;
    });
  // страховка: если событие refresh.active=false не придёт, спиннер не зависнет
  setTimeout(() => {
    if (!state.snapshot?.auth?.loggedIn) return;
    setBusy('refresh', false);
    if (state.view === 'drops') render();
  }, 20000);
}

function setBusy(key, value) {
  if (value) state.busy.add(key);
  else state.busy.delete(key);
}

function isBusy(key) {
  return state.busy.has(key);
}

/* -------------------------------------------------------------------- data */

function demoSnapshot() {
  const now = Date.now();
  const iso = (msFromNow) => new Date(now + msFromNow).toISOString();

  const make = (game, campaign, drops) => ({
    id: `${game.slug}-${campaign}`,
    name: campaign,
    status: 'ACTIVE',
    game: {
      id: game.id,
      name: game.name,
      slug: game.slug,
      boxArtUrl: `https://static-cdn.jtvnw.net/ttv-boxart/${game.id}-285x380.jpg`,
    },
    startsAt: iso(-86400000 * 3),
    endsAt: iso(game.endsIn),
    linked: true,
    allowedChannelCount: 0,
    selected: game.selected ?? false,
    farming: game.slug === 'fortnite',
    drops: drops.map((drop, index) => {
      const required = drop.required;
      const current = drop.current;
      const done = current >= required;
      return {
        id: `${game.slug}-${campaign}-${index}`,
        name: drop.name,
        currentMinutes: current,
        requiredMinutes: required,
        remainingMinutes: Math.max(0, required - current),
        progress: clamp01(current / required),
        isClaimed: Boolean(drop.claimed),
        canClaim: done && !drop.claimed,
        startsAt: iso(-86400000 * 3),
        endsAt: iso(game.endsIn),
        benefits: [{ id: `${index}`, name: drop.name, type: 'DIRECT_ENTITLEMENT', imageUrl: null }],
      };
    }),
  });

  const games = [
    { id: 33214, name: 'Fortnite', slug: 'fortnite', endsIn: 86400000 * 4, selected: true },
    { id: 512710, name: 'Call of Duty: Warzone', slug: 'call-of-duty-warzone', endsIn: 86400000 * 11, selected: true },
    { id: 21779, name: 'League of Legends', slug: 'league-of-legends', endsIn: 86400000 * 2 },
    { id: 516575, name: 'VALORANT', slug: 'valorant', endsIn: 86400000 * 7 },
    { id: 511224, name: 'Apex Legends', slug: 'apex-legends', endsIn: 86400000 * 1 },
    { id: 263490, name: 'Rust', slug: 'rust', endsIn: 86400000 * 18 },
    { id: 32982, name: 'Grand Theft Auto V', slug: 'grand-theft-auto-v', endsIn: 86400000 * 6 },
    { id: 27471, name: 'Minecraft', slug: 'minecraft', endsIn: 86400000 * 9 },
  ];

  const campaigns = [
    make(games[0], 'Fortnite Drops: Remix', [
      { name: 'Эмоция «Rift Rider»', required: 30, current: 30, claimed: true },
      { name: 'Облик «Neon Drift»', required: 60, current: 47 },
      { name: 'Кирка «Starforge»', required: 120, current: 12 },
    ]),
    make(games[1], 'Warzone: Season Rewards', [
      { name: 'Чертёж «Blackout»', required: 60, current: 60 },
      { name: 'Визитка «Ghost»', required: 180, current: 64 },
    ]),
    make(games[2], 'LoL: Prime Gaming', [
      { name: 'Сундук с наградой', required: 45, current: 45, claimed: true },
      { name: 'Эмблема «Rift»', required: 90, current: 21 },
    ]),
    make(games[3], 'VALORANT: Champion Drops', [
      { name: 'Спрей «Ace»', required: 30, current: 30, claimed: true },
      { name: 'Карточка «Lotus»', required: 60, current: 8 },
      { name: 'Титул «Clutch»', required: 90, current: 0 },
    ]),
    make(games[4], 'Apex: Legends Rewards', [{ name: 'Облик «Voidwalker»', required: 120, current: 96 }]),
    make(games[5], 'Rust: Twitch Drops', [
      { name: 'Скин «Molten»', required: 240, current: 120 },
      { name: 'Ящик «Scrap»', required: 60, current: 60, claimed: true },
    ]),
    make(games[6], 'GTA Online: Rewards', [{ name: 'Футболка «Neon»', required: 60, current: 33 }]),
    make(games[7], 'Minecraft: Creator Drops', [{ name: 'Плащ «Enderman»', required: 45, current: 45, claimed: true }]),
  ];

  const gameSummary = new Map();
  for (const campaign of campaigns) {
    const key = campaign.game.slug;
    if (!gameSummary.has(key)) {
      gameSummary.set(key, {
        slug: key,
        name: campaign.game.name,
        boxArtUrl: campaign.game.boxArtUrl,
        campaignIds: [],
        totalDrops: 0,
        claimedDrops: 0,
        endsAt: campaign.endsAt,
        active: true,
        canEarn: true,
        selected: games.find((game) => game.slug === key)?.selected ?? false,
        farming: key === 'fortnite',
      });
    }
    const summary = gameSummary.get(key);
    summary.campaignIds.push(campaign.id);
    summary.totalDrops += campaign.drops.length;
    summary.claimedDrops += campaign.drops.filter((drop) => drop.isClaimed).length;
  }

  const allDrops = campaigns.flatMap((campaign) => campaign.drops);

  return {
    ready: true,
    demo: true,
    auth: { loggedIn: true, user: { login: 'dropforge_demo', displayName: 'dropforge_demo' }, profile: 'web' },
    status: {
      state: 'watching',
      running: true,
      message: 'Смотрю стрим',
      game: 'Fortnite',
      channel: { login: 'ninja_live', displayName: 'NinjaLive', viewers: 1240, url: 'https://twitch.tv/ninja_live' },
    },
    pubsub: 'online',
    lastRefresh: new Date().toISOString(),
    settings: { games: ['fortnite', 'call-of-duty-warzone'], sendSpadeEvents: true, autoClaim: true, refreshMinutes: 10 },
    stats: {
      games: gameSummary.size,
      campaigns: campaigns.length,
      drops: allDrops.length,
      claimed: allDrops.filter((drop) => drop.isClaimed).length,
      claimable: allDrops.filter((drop) => drop.canClaim).length,
      minutesEarned: allDrops.reduce((sum, drop) => sum + drop.currentMinutes, 0),
    },
    games: [...gameSummary.values()],
    campaigns,
    logs: [
      { at: new Date(now - 120000).toISOString(), level: 'info', message: 'Загружено кампаний: 8, дропов: 19' },
      { at: new Date(now - 90000).toISOString(), level: 'success', message: 'Награда получена: Сундук с наградой (League of Legends)' },
      { at: new Date(now - 60000).toISOString(), level: 'info', message: 'Смотрю ninja_live — прогресс +1 мин' },
    ],
  };
}

/* ---------------------------------------------------------------- auth view */

function deviceOverlay() {
  return `
  <div class="device-overlay">
    <div class="device-card">
      <h3>Переподключение Twitch</h3>
      <p class="row-hint">Открой страницу подтверждения и введи код. Профиль и настройки не сбросятся.</p>
      <div class="device-code">
        <div class="device-label">Код подтверждения</div>
        <div class="code">${esc(state.device.userCode ?? '')}</div>
        <div class="device-status"><span class="spinner"></span> Ждём подтверждения в браузере…</div>
      </div>
      <div style="display:flex;gap:8px;margin-top:12px">
        <button class="btn-primary" data-action="open-activation">Открыть страницу Twitch</button>
        <button class="btn-ghost" data-action="cancel-device">Отмена</button>
      </div>
    </div>
  </div>`;
}

function authScreen() {
  const error = state.error ? `<div class="error-box">${esc(state.error)}</div>` : '';
  const device = state.device
    ? `<div class="device-code">
         <div class="device-label">Код подтверждения</div>
         <div class="code">${esc(state.device.userCode)}</div>
         <div class="device-status"><span class="spinner"></span> Ждём подтверждения в браузере…</div>
         <div class="hint">
           Браузер уже открыт. Если нет —
           <button type="button" class="link" data-action="open-activation">откройте twitch.tv/activate</button>
           и введите код.
         </div>
         <button type="button" class="btn-ghost small" data-action="cancel-device">Отмена</button>
       </div>`
    : '';

  return `
  <div class="auth-screen">
    <div class="auth-left">
      <div class="auth-brand">
        <div class="brand-mark">${ICONS.logo}</div>
        <div class="brand-text">
          <span class="brand-name">DropForge</span>
          <span class="brand-sub">Twitch Drops automation</span>
        </div>
      </div>
      <h1 class="auth-title">Дропы копятся, <em>пока вы заняты</em> другими делами</h1>
      <p class="auth-lead">
        Выберите игру — приложение само найдёт стрим с включёнными Drops, посмотрит его за вас
        и заберёт награды. Держать вкладки открытыми не нужно.
      </p>
      <div class="auth-points">
        <div class="auth-point">${ICONS.drops}<span><b>Любая игра с Drops.</b> Кампании и прогресс подтягиваются из вашего аккаунта.</span></div>
        <div class="auth-point">${ICONS.bolt}<span><b>Смена стримера на лету.</b> Стрим закончился — приложение переключится само.</span></div>
        <div class="auth-point">${ICONS.gift}<span><b>Награды забираются автоматически.</b> Как только дроп прокнул — он уже ваш.</span></div>
        <div class="auth-point">${ICONS.shield}<span><b>Вход только на сайте Twitch.</b> Приложение не спрашивает пароль и не видит его.</span></div>
      </div>
    </div>
    <div class="auth-right">
      <form class="auth-card" data-action="login-token">
        <h2>Подключите аккаунт Twitch</h2>
        <p class="small">Нажмите «Войти» — откроется браузер, где вы уже вошли (или войдёте) на Twitch.</p>
        ${error}
        <button type="button" class="btn-primary wide" data-action="login-device" ${state.device ? 'disabled' : ''}>
          ${ICONS.play} Войти через Twitch
        </button>
        <p class="auth-note">Подтвердите вход на странице Twitch — приложение получит доступ само, пароль вводить в него не нужно.</p>
        ${device}
        <details class="auth-advanced">
          <summary>Вход по токену</summary>
          <div class="field">
            <label for="token">OAuth-токен</label>
            <input id="token" name="token" placeholder="Вставьте токен из cookie auth-token" autocomplete="off" spellcheck="false" />
          </div>
          <button type="submit" class="btn-ghost wide">${ICONS.shield} Войти по токену</button>
        </details>
      </form>
    </div>
  </div>`;
}

/* ------------------------------------------------------------------- shell */

/** Очередь игр в панели автофарма — с возможностью убрать игру. */
function queueBlock() {
  const queue = [...state.snapshot.games].filter((game) => game.selected);
  const running = Boolean(state.snapshot.status?.running);
  const head = `
    <div class="queue-head">
      Очередь <span class="queue-count">${queue.length}</span>
      <div class="queue-actions">
        <button class="queue-icon toggle ${running ? 'stop' : 'start'}" data-action="${running ? 'stop' : 'start'}" title="${running ? 'Поставить на паузу' : 'Запустить автофарм'}" ${!running && !queue.length ? 'disabled' : ''}>${running ? ICONS.pause : ICONS.play}</button>
        <button class="queue-icon danger" data-action="clear-queue" title="Очистить очередь" ${queue.length ? '' : 'disabled'}>${ICONS.trash}</button>
      </div>
    </div>`;

  if (!queue.length) {
    return `
  <div class="queue">
    ${head}
    <div class="queue-empty">Очередь пуста — добавьте игру из списка</div>
  </div>`;
  }

  const items = queue
    .map(
      (game) => `
    <li class="queue-item" title="${esc(game.name)}">
      <span class="queue-dot ${game.farming ? 'live' : ''}"></span>
      <span class="queue-name">${esc(game.name)}</span>
      <button class="queue-remove" data-action="toggle-game" data-slug="${esc(game.slug)}" title="Убрать из очереди">${ICONS.close}</button>
    </li>`,
    )
    .join('');

  return `
  <div class="queue">
    ${head}
    <ul class="queue-list">${items}</ul>
  </div>`;
}

/** Мини-панель прогресса текущего фарма — без открытия карточки игры. */
function liveProgress() {
  const snapshot = state.snapshot;
  const campaign = snapshot.campaigns.find((item) => item.farming) ?? null;
  const channel = snapshot.status?.channel ?? null;

  if (!campaign) {
    return `
  <div class="live-card idle">
    <div class="live-head">Прогресс</div>
    <div class="live-empty">Фарм не запущен<br /><span>Выберите игру и нажмите «Фармить»</span></div>
  </div>`;
  }

  const collection = (state.snapshot.collections ?? []).find((item) => item.gameSlug === campaign.game.slug);
  const drops = [...campaign.drops]
    .filter((drop) => Number(drop.requiredSubs ?? 0) <= 0)
    .sort((a, b) => a.requiredMinutes - b.requiredMinutes);
  const active =
    drops.find((drop) => !drop.isClaimed && drop.progress < 1) ?? drops[drops.length - 1] ?? null;
  const next = active ? drops.find((drop) => !drop.isClaimed && drop.id !== active.id) : null;
  const remaining = active ? Math.max(0, active.requiredMinutes - active.currentMinutes) : 0;
  const percent = active ? Math.round(Math.min(1, active.progress) * 100) : 0;
  const nextChainName = (collection?.chains ?? []).find((chain) => !chain.earned)?.name ?? '';

  return `
  <div class="live-card" data-action="open-game" data-slug="${esc(campaign.game.slug)}" role="button" tabindex="0" title="Открыть карточку игры">
    <div class="live-head">Сейчас в фарме</div>
    <div class="live-game">
      ${campaign.game.boxArtUrl ? `<img class="live-art" src="${esc(campaign.game.boxArtUrl)}" alt="" loading="lazy" data-fallback />` : ''}
      <div class="live-meta">
        <div class="live-name">${esc(campaign.game.name)}</div>
        <div class="live-status">
          <span class="live-dot"></span>
          ${channel ? esc(channel.login) : 'ищу стрим…'}
        </div>
      </div>
    </div>

    ${
      active
        ? `
    <div class="live-drop">
      <div class="live-drop-title">
        ${active.benefits?.[0]?.imageUrl ? `<img class="live-drop-art" src="${esc(active.benefits[0].imageUrl)}" alt="" loading="lazy" data-fallback />` : ''}
        <div class="live-drop-name" title="${esc(collection ? `Ежедневный сундук · цепочек ${collection.chests}/${collection.chestsTotal}` : active.name)}">${esc(collection ? `Ежедневный сундук · цепочек ${collection.chests}/${collection.chestsTotal}` : active.name)}</div>
      </div>
      ${collection ? '' : `<div class="bar ${active.isClaimed ? 'ok' : ''}"><i style="width:${percent}%"></i></div>`}
      <div class="live-row">
        <span class="live-minutes">${collection ? `цепочки ${collection.chests}/${collection.chestsTotal} · подписки ${collection.subs}/${collection.subsTotal}` : `${active.currentMinutes}/${active.requiredMinutes} мин`}</span>
        ${collection ? '' : `<span class="live-percent">${percent}%</span>`}
      </div>
      <div class="live-row">
        <span class="live-left">${collection
          ? (nextChainName ? `следующая цепочка: ${esc(nextChainName)}` : 'все цепочки получены')
          : (remaining > 0 ? `ещё ~${formatMinutes(remaining)}` : 'готово к получению')}</span>
        <span class="live-left">${campaign.endsAt ? 'до ' + formatDate(campaign.endsAt) : ''}</span>
      </div>
    </div>`
        : ''
    }

  </div>`;
}

function sidebar() {
  const snapshot = state.snapshot;
  const { stats, status, auth } = snapshot;
  const running = Boolean(status?.running);
  const readyGames = snapshot.settings?.games?.length ?? 0;
  const badgesFree = (snapshot.badges ?? []).filter(
    (badge) => badge.available && !badge.claimed,
  ).length;

  return `
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">${ICONS.logo}</div>
      <div class="brand-text">
        <span class="brand-name">DropForge</span>
        <span class="brand-sub">Twitch Drops</span>
      </div>
    </div>

    <nav class="nav">
      <button class="nav-item ${state.view === 'drops' ? 'active' : ''}" data-action="nav" data-view="drops">
        ${ICONS.drops} Дропы ${stats.claimable ? `<span class="nav-badge">${stats.claimable}</span>` : ''}
      </button>
      <button class="nav-item ${state.view === 'badges' ? 'active' : ''}" data-action="nav" data-view="badges">
        ${ICONS.gift} Бейджи ${badgesFree ? `<span class="nav-badge">${badgesFree}</span>` : ''}
      </button>
      <button class="nav-item ${state.view === 'streaks' ? 'active' : ''}" data-action="nav" data-view="streaks">
        ${ICONS.flame} Стрики
      </button>
      <button class="nav-item ${state.view === 'settings' ? 'active' : ''}" data-action="nav" data-view="settings">
        ${ICONS.gear} Настройки
      </button>
    </nav>

    ${liveProgress()}

    <div class="sidebar-spacer"></div>

    <div class="farm-card">
      ${queueBlock()}
    </div>

    <div class="account">
      <div class="avatar-wrap">
        <div class="avatar">${esc(initials(auth?.user?.login))}</div>
        ${auth?.user?.avatarUrl
          ? `<img class="avatar-img" src="${esc(auth.user.avatarUrl)}" alt="" loading="lazy" onerror="this.remove()" />`
          : ''}
      </div>
      <div class="account-meta">
        <div class="account-name">${esc(auth?.user?.login ?? 'не подключён')}</div>
      </div>
      <button class="icon-btn" title="Выйти" data-action="logout">${ICONS.logout}</button>
    </div>
  </aside>`;
}

function topbar() {
  const snapshot = state.snapshot;
  const title =
    state.view === 'drops'
      ? 'Дропы'
      : state.view === 'badges'
        ? 'Бейджи'
        : state.view === 'streaks'
          ? 'Стрики'
          : state.view === 'logs'
            ? 'Журнал'
            : 'Настройки';
  const subtitle =
    state.view === 'streaks'
      ? 'Серии просмотра каналов из подписок'
      : state.view === 'drops'
      ? `${snapshot.stats.games} игр · ${snapshot.stats.campaigns} кампаний · ${snapshot.stats.drops} дропов`
      : state.view === 'badges'
        ? `Значки профиля Twitch · ${(snapshot.badges ?? []).filter((badge) => badge.claimed).length} из ${(snapshot.badges ?? []).length} получено`
        : state.view === 'logs'
        ? 'Что делает приложение прямо сейчас'
        : 'Поведение автофарма и аккаунт';

  const search =
    state.view === 'drops'
      ? `<div class="search">${ICONS.search}<input id="search" placeholder="Поиск игры" value="${esc(state.query)}" />${state.query ? '<button class="search-clear" data-action="clear-search" title="Сбросить фильтр"><svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M1.5 1.5 8.5 8.5M8.5 1.5 1.5 8.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg></button>' : ''}</div>`
      : '';

  const refresh =
    state.view === 'drops'
      ? `<button class="btn-ghost" data-action="refresh" ${isBusy('refresh') ? 'disabled' : ''}>
           ${isBusy('refresh') ? '<span class="spinner"></span>' : ICONS.refresh} Обновить
         </button>`
      : '';

  return `
  <header class="topbar">
    <div>
      <h1 class="page-title">${title}</h1>
      <div class="page-sub">${subtitle}</div>
    </div>
    <div class="topbar-actions">
      ${search}
      ${refresh}
    </div>
  </header>`;
}

/* -------------------------------------------------------------- drops view */

/** Кампании активной вкладки: глобальные или локальные (привязанные к каналу). */
function tabCampaigns() {
  const campaigns = state.snapshot.campaigns ?? [];
  const local = state.dropsTab === 'local';
  if (local) {
    const checked = state.snapshot.settings?.localCheck;
    if (!checked?.login) return [];
    return campaigns.filter((campaign) => campaign.local && campaign.channel?.login === checked.login);
  }
  return campaigns.filter((campaign) => Boolean(campaign.local) === local);
}

/** Группирует кампании вкладки по игре — карточки строятся из них. */
function groupedGames() {
  const base = new Map((state.snapshot.games ?? []).map((game) => [game.slug, game]));
  const bySlug = new Map();
  for (const campaign of tabCampaigns()) {
    const slug = campaign.game?.slug;
    if (!slug) continue;
    let game = bySlug.get(slug);
    if (!game) {
      game = {
        slug,
        name: campaign.game.name,
        boxArtUrl: campaign.game.boxArtUrl,
        totalDrops: 0,
        claimedDrops: 0,
        endsAt: null,
        canEarn: false,
        channels: new Set(),
      };
      bySlug.set(slug, game);
    }
    game.totalDrops += campaign.drops.length;
    game.claimedDrops += campaign.drops.filter((drop) => drop.isClaimed).length;
    game.canEarn = game.canEarn || Boolean(campaign.canEarn);
    const end = campaign.endsAt ? new Date(campaign.endsAt) : null;
    if (end && (!game.endsAt || end < game.endsAt)) game.endsAt = end;
    if (campaign.channel?.login) game.channels.add(campaign.channel.login);
    if (campaign.channel?.login) game.channel = campaign.channel.login;
  }
  return [...bySlug.values()].map((game) => ({
    ...(base.get(game.slug) ?? {}),
    ...game,
    // локальная карточка не «награда за подписку» — показываем обычный статус дропсов
    rewardOnly: false,
    channels: [...game.channels],
  }));
}

function sortedGames() {
  return groupedGames().sort(
    (a, b) =>
      Number(Boolean(b.pinned)) - Number(Boolean(a.pinned)) ||
      Number(b.farming) - Number(a.farming) ||
      Number(b.canEarn) - Number(a.canEarn) ||
      a.name.localeCompare(b.name),
  );
}

function matchesQuery(game, query) {
  return !query || game.name.toLowerCase().includes(query);
}

/** Один канал: проверка существования и локальных дропсов. */
function localChannelsBlock() {
  const settings = state.snapshot.settings ?? {};
  const current = settings.localChannel ?? '';
  const checked = settings.localCheck;
  const foundNow = (state.snapshot.campaigns ?? []).filter((campaign) => campaign.local && campaign.channel?.login === current).length;
  const chip = current
    ? `<span class="channel-chip">${esc(current)}<button class="channel-remove" data-action="local-clear" title="Сбросить канал">${ICONS.close}</button></span>`
    : '';
  let notice = '';
  if (checked && foundNow > 0) {
    notice = `<div class="local-note ok">Канал <b>${esc(checked.login)}</b>: найдено кампаний — ${foundNow}.</div>`;
  } else if (checked) {
    const at = checked.at ? new Date(checked.at).toLocaleTimeString('ru-RU') : '';
    notice = `<div class="local-note warn">У канала <b>${esc(checked.login)}</b> сейчас нет локальных дропсов.${at ? ` Проверено в ${at}.` : ''}</div>`;
  }
  return `
  <div class="channels-row">
    ${chip || '<span class="channels-empty">Проверьте канал — покажем его локальные дропсы</span>'}
    <form class="channel-form" data-action="local-check">
      <input name="login" placeholder="логин канала, например streamdatabase" autocomplete="off" spellcheck="false" value="${esc(current)}" />
      <button class="btn-ghost small" type="submit">Проверить</button>
    </form>
  </div>
  ${notice}`;
}

/**
 * Сетка игр. Карточки создаются один раз; при поиске мы только прячем
 * лишние — иначе на каждую букву пересоздавались бы элементы и картинки,
 * из-за чего сетка дёргалась.
 */
function localSection() {
  return `
    <div class="empty" style="min-height:420px;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center">
      <h3 style="font-size:18px">В разработке</h3>
      <p>Локальные дропсы пока недоступны — раздел в разработке.</p>
    </div>`;
}

function gamesSection() {
  const local = state.dropsTab === 'local';
  const games = sortedGames();
  const query = state.query.trim().toLowerCase();
  const filtered = games.filter((game) => matchesQuery(game, query));
  const total = filtered.length;
  // страница = 4 полных ряда карточек; следующая начинается после заполнения
  const gridWidth = (() => {
    const content = document.getElementById('content');
    if (content) return content.clientWidth - 48;
    return Math.max(320, window.innerWidth - 268 - 48);
  })();
  const columns = Math.max(1, Math.floor((gridWidth + 14) / (196 + 14)));
  const ROWS = 4;
  const perPage = Math.max(1, columns * ROWS);
  const pageCount = Math.max(1, Math.ceil(total / perPage));
  const page = Math.min(Math.max(Number(state.page) || 0, 0), pageCount - 1);
  state.page = page;
  const start = page * perPage;
  const slice = filtered.slice(start, start + perPage);
  const visible = total;
  const cards = slice.map((game) => gameCard(game)).join('');
  const pager = total > 0
    ? '<div class="pager">' + Array.from({ length: pageCount }, (_, index) =>
        '<button class="pager-btn ' + (index === page ? 'active' : '') + '" data-action="page" data-page="' + index + '">' + (index + 1) + '</button>',
      ).join('') + '</div>'
    : '';


  return `
  ${local ? localChannelsBlock() : ''}
  <div class="section-head">
    <h2 class="section-title">${local ? 'Локальные дропсы каналов' : 'Игры с дропами'}</h2>
    <span class="section-hint" id="games-hint">${visible} ${visible === 1 ? 'игра' : 'игр'} · нажмите на карточку, чтобы увидеть награды</span>
    ${local ? '' : '<button class="btn-ghost small" style="margin-left:auto" data-action="select-farmable">Выбрать всё доступное</button>'}
  </div>

  <div class="games-grid" id="games-grid" ${visible ? '' : 'hidden'}>${cards}</div>
  ${pager}
  <div class="empty" id="games-empty" ${visible ? 'hidden' : ''}>
    ${local
      ? '<h3>Локальных дропсов нет</h3><p>Проверьте канал выше — приложение покажет кампании, которые Twitch выдаёт только на нём.</p>'
      : '<h3>Ничего не найдено</h3><p>Измените запрос или обновите список — возможно, кампании ещё не начались.</p>' + (query ? '<button class="btn-ghost" data-action="clear-search">Сбросить фильтр</button>' : '')}
  </div>`;
}

function catalogNotice(snapshot) {
  const catalog = snapshot.catalog;
  if (!catalog) return '';
  if (catalog.stale) {
    const at = catalog.at ? new Date(catalog.at).toLocaleString('ru-RU') : 'прошлого запуска';
    return `<div class="catalog-note warn">Источники каталога недоступны — список взят из кэша (${esc(at)})</div>`;
  }
  if (!catalog.ok) {
    return `<div class="catalog-note warn">Каталог недоступен${catalog.message ? `: ${esc(catalog.message)}` : ''}. Показаны только кампании из вашего аккаунта.</div>`;
  }
  return '';
}

function dropsView() {
  const snapshot = state.snapshot;
  const local = state.dropsTab === 'local';
  const campaigns = snapshot.campaigns ?? [];
  const globalCount = campaigns.filter((campaign) => !campaign.local).length;
  const checkedLogin = snapshot.settings?.localCheck?.login;
  const localCount = checkedLogin
    ? campaigns.filter((campaign) => campaign.local && campaign.channel?.login === checkedLogin).length
    : 0;

  return `
  <div class="stats-row">
    <div class="stat"><div class="stat-label">Игры</div><div class="stat-value">${snapshot.stats.games}</div></div>
    <div class="stat"><div class="stat-label">Дропы</div><div class="stat-value">${snapshot.stats.claimed}<small> / ${snapshot.stats.drops}</small></div></div>
    <div class="stat"><div class="stat-label">Готовы к выдаче</div><div class="stat-value">${snapshot.stats.claimable}</div></div>
    <div class="stat"><div class="stat-label">Накоплено</div><div class="stat-value">${formatMinutes(snapshot.stats.minutesEarned)}</div></div>
    <div class="stat" style="margin-left:auto"><div class="stat-label">Обновлено</div><div class="stat-value" style="font-size:13px">${snapshot.lastRefresh ? new Date(snapshot.lastRefresh).toLocaleTimeString('ru-RU') : '—'}</div></div>
  </div>

  <div class="tabs" role="tablist">
    <button class="tab ${state.dropsTab === 'global' ? 'active' : ''}" data-action="drops-tab" data-tab="global" role="tab">
      Глобальные <span class="tab-count">${globalCount}</span>
    </button>
    <button class="tab ${state.dropsTab === 'local' ? 'active' : ''}" data-action="drops-tab" data-tab="local" role="tab">
      Локальные <span class="tab-count">${localCount}</span>
    </button>
  </div>

  ${catalogNotice(snapshot)}

  ${local ? localSection() : `<div id="games-section">${gamesSection()}</div>`}`;
}

const CARD_CACHE = new Map();

function gameCard(game, { hidden = false } = {}) {
  const art = boxArt(game);
  const collection =
    (state.snapshot.collections ?? []).find((item) => item.gameSlug === game.slug) ??
    (state.snapshot.collections ?? []).find((item) => item.campaign === game.name);
  const totalDrops = collection ? collection.chestsTotal + collection.subsTotal : game.totalDrops;
  const claimedDrops = collection ? collection.chests + collection.subs : game.claimedDrops;
  const ratio = totalDrops ? claimedDrops / totalDrops : 0;
  const ends = game.endsAt ? new Date(game.endsAt).getTime() - Date.now() : Infinity;
  const soon = ends < 48 * 3600 * 1000;

  const signature = JSON.stringify([
    hidden,
    game.slug,
    game.name,
    game.totalDrops,
    game.claimedDrops,
    Boolean(game.canEarn),
    Boolean(game.selected),
    Boolean(game.pinned),
    game.endsAt ?? null,
    game.boxArtUrl ?? null,
  ]);
  const cached = CARD_CACHE.get(game.slug);
  if (cached && cached.signature === signature) {
    return cached.html;
  }
  const html = `
  <div class="game-card ${game.selected ? 'selected' : ''}" data-action="open-game" data-slug="${esc(game.slug)}" data-name="${esc(game.name.toLowerCase())}" role="button" tabindex="0" ${hidden ? 'hidden' : ''}>
    <div class="game-art">
      ${art ? `<img src="${esc(art)}" alt="" loading="lazy" data-fallback />` : ''}
      <div class="game-badges">
        ${game.rewardOnly ? '<span class="badge">награда</span>' : game.canEarn ? '<span class="badge live"><i></i>drops</span>' : '<span class="badge">пауза</span>'}
        ${game.channel ? `<span class="badge">канал: ${esc(game.channel)}</span>` : ''}
        ${game.farming ? '<span class="badge farm">фарм</span>' : ''}
      </div>
    </div>
    <div class="game-info">
      <div class="game-title" title="${esc(game.name)}">${esc(game.name)}</div>
      <div class="game-progress">
        <span class="bar ${ratio >= 1 ? 'ok' : ''}"><i style="width:${percent(ratio)}"></i></span>
        <span class="count">${claimedDrops}/${totalDrops}</span>
      </div>
      <div class="game-foot">
        <span class="game-ends ${soon ? 'soon' : ''}">${soon ? 'осталось ' + esc(formatLeft(new Date(game.endsAt).toISOString())) : 'до ' + esc(formatDate(game.endsAt))}</span>
        <div class="game-actions">
          <button class="pin ${game.pinned ? 'on' : ''}" data-action="pin-game" data-slug="${esc(game.slug)}" title="${game.pinned ? 'Открепить' : 'Закрепить сверху'}">
            ${game.pinned ? ICONS.starOn : ICONS.star}
          </button>
          <button class="pick ${game.selected ? 'on' : ''}" data-action="toggle-game" data-slug="${esc(game.slug)}" title="${game.selected ? 'Убрать из фарма' : 'Добавить в фарм'}">
            ${game.selected ? ICONS.check : ICONS.plus}
          </button>
        </div>
      </div>
    </div>
  </div>`;
  CARD_CACHE.set(game.slug, { signature, html });
  return html;
}

/* ------------------------------------------------------------- detail sheet */

function sheet() {
  const snapshot = state.snapshot;
  const game = snapshot.games.find((item) => item.slug === state.sheetGame);
  if (!game) return '<div class="sheet-backdrop" data-action="close-sheet"></div><aside class="sheet" id="sheet"></aside>';

  const campaigns = snapshot.campaigns.filter(
    (campaign) => campaign.game.slug === game.slug && !campaign.expired,
  );

  const body = campaigns.length
    ? campaigns.map(campaignBlock).join('')
    : '<div class="empty"><h3>Кампаний нет</h3><p>Для этой игры сейчас не проводятся Drops.</p></div>';

  const collection = (state.snapshot.collections ?? []).find((item) => item.gameSlug === game.slug);
  const totalDrops = collection ? collection.chestsTotal + collection.subsTotal : campaigns.reduce((sum, campaign) => sum + campaign.drops.length, 0);
  const claimed = collection ? collection.chests + collection.subs : campaigns.reduce(
    (sum, campaign) => sum + campaign.drops.filter((drop) => drop.isClaimed).length,
    0,
  );

  return `
  <div class="sheet-backdrop open" data-action="close-sheet"></div>
  <aside class="sheet open" id="sheet">
    <div class="sheet-hero">
      <button class="icon-btn sheet-close" data-action="close-sheet">${ICONS.close}</button>
      <div class="sheet-game">
        ${boxArt(game) ? `<img class="sheet-art" src="${esc(boxArt(game))}" alt="" data-fallback />` : ''}
        <div>
          <h2 class="sheet-title">${esc(game.name)}</h2>
          <div class="sheet-meta">${claimed}/${totalDrops} дропов · до ${esc(formatLeft(game.endsAt))}</div>
        </div>
      </div>
      <div class="sheet-actions">
        ${game.rewardOnly
          ? `<button class="btn-primary ${game.farming ? 'danger' : ''}" style="flex:1" data-action="${game.farming ? 'stop' : 'farm-game'}" data-slug="${esc(game.slug)}">
          ${game.farming ? ICONS.stop + ' Остановить' : ICONS.play + ' Смотреть'}
        </button>
        <button class="btn-ghost" data-action="open-rewards" title="Открыть страницу наград Twitch">Забрать на Twitch</button>`
          : `<button class="btn-primary ${game.farming ? 'danger' : ''}" style="flex:1" data-action="${game.farming ? 'stop' : 'farm-game'}" data-slug="${esc(game.slug)}">
          ${game.farming ? ICONS.stop + ' Остановить' : ICONS.play + ' Фармить'}
        </button>
        <button class="btn-ghost" data-action="toggle-game" data-slug="${esc(game.slug)}" title="${game.selected ? 'Убрать из списка фарма' : 'Добавить в список, не запуская'}">
          ${game.selected ? ICONS.check + ' В списке' : ICONS.plus + ' В список'}
        </button>`}
      </div>
    </div>
    <div class="sheet-body">${body}</div>
  </aside>`;
}

function renderGtaDay(collection) {
  const day = collection.gtaDay ?? {};
  const watched = Math.round(Number(day.watched ?? 0));
  const started = day.startedAt;
  const completed = day.completedAt;
  const nextAt = completed ? new Date(completed).getTime() + 24 * 60 * 60 * 1000 : 0;
  const now = Date.now();
  if (started && !completed && watched < 60) {
    return `<div style="margin-top:6px">Идёт день: ${watched}/60 мин · следующий сундук откроется через 24 часа после закрытия</div>`;
  }
  if (completed && nextAt > now) {
    const left = Math.ceil((nextAt - now) / 60000);
    const hours = Math.floor(left / 60);
    const mins = left % 60;
    return `<div style="margin-top:6px">Сундук дня получен · следующий день через ${hours} ч ${mins} мин <button class="btn-ghost small" data-action="gta-start-day">Начать раньше</button></div>`;
  }
  return `<div style="margin-top:6px">Доступен следующий день для просмотра — <button class="btn-primary small" data-action="gta-start-day">Начать день</button> · отсчёт пойдёт с этого момента</div>`;
}

function campaignBlock(campaign) {
  const collection = (state.snapshot.collections ?? []).find((item) => item.campaignId === campaign.id);
  const isGta = /gtavi/i.test(campaign.name ?? '');
  const watchDrops = campaign.drops
    .filter((drop) => Number(drop.requiredSubs ?? 0) <= 0)
    .map((drop) => {
      if (isGta && /сундук/i.test(drop.name ?? '')) {
        const done = Number(collection?.chests ?? 0);
        return { ...drop, name: `Ежедневный сундук с цепочкой · получено ${done} из 6`, note: 'Проверяйте прогресс на Twitch' };
      }
      return drop;
    });
  const subDrops = campaign.drops
    .filter((drop) => Number(drop.requiredSubs ?? 0) > 0)
    .map((drop) => ({
      ...drop,
      earned: (collection?.earnedNames ?? []).some((name) => name === drop.name),
    }));
  const groupHead = (text) =>
    `<div style="margin:14px 2px 8px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;opacity:.65">${text}</div>`;
  const drops = [
    watchDrops.length ? groupHead('Смотреть Drops') + watchDrops.map(dropRow).join('') : '',
    subDrops.length ? groupHead('Drops для подписки') + subDrops.map(dropRow).join('') : '',
  ].join('');
  const status = campaign.reward
    ? '<span class="badge soon">награда Twitch</span>'
    : campaign.upcoming
      ? `<span class="badge soon">старт ${esc(formatDate(campaign.startsAt))}</span>`
      : campaign.completed
        ? '<span class="badge live">всё получено</span>'
        : '';
  const notes = [];
  if (campaign.reward) {
    notes.push('<p class="campaign-note">Награда за просмотр категории: часы идут в любой трансляции игры. Прогресс считает Twitch, а забирается награда на twitch.tv/drops/rewards.</p>');
  }
  if (campaign.linkUrl) {
    notes.push(`<p class="campaign-note">Нужна привязка аккаунта: без неё Twitch не засчитывает минуты. <button class="btn-ghost small" data-action="open-url" data-url="${esc(campaign.linkUrl)}">Привязать аккаунт</button></p>`);
  }
  const note = notes.join('');
  const collectionNote = collection
    ? `<div class="campaign-note">
        <div>Коллекция: сундуки ${collection.chests}/${collection.chestsTotal} · подписки ${collection.subs}/${collection.subsTotal} · финальная награда: ${collection.finalEarned ? 'получена' : 'нет'}</div>
        ${collection.gtaDay !== undefined ? renderGtaDay(collection) : ''}
        ${collection.chains?.length
          ? `<div style="margin-top:8px;display:flex;gap:12px;flex-wrap:wrap">${collection.chains
              .map((chain) => `<span style="display:inline-flex;align-items:center;gap:6px;opacity:${chain.earned ? 1 : 0.4}">${chain.imageUrl ? `<img src="${esc(chain.imageUrl)}" alt="" style="width:22px;height:22px" />` : (chain.earned ? '✔' : '✖')} ${esc(chain.name)}</span>`)
              .join('')}</div>`
          : ''}
      </div>`
    : '';
  return `
  <section class="campaign">
    <div class="campaign-head">
      <div class="campaign-name">${esc(campaign.name)}</div>
      ${status}
      <div class="campaign-time">до ${esc(formatDate(campaign.endsAt))}</div>
    </div>
    ${note}
    ${collectionNote}
    ${drops}
  </section>`;
}

function subsWord(count) {
  const rest = count % 100;
  const last = count % 10;
  if (rest >= 11 && rest <= 14) return 'подписок';
  if (last === 1) return 'подписка';
  if (last >= 2 && last <= 4) return 'подписки';
  return 'подписок';
}

function dropRow(drop) {
  const subs = Number(drop.requiredSubs ?? 0);
  const note = String(drop.note ?? "").trim();
  const label = drop.earned
    ? '<span class="drop-status done">получено</span>'
    : note && !drop.isClaimed
    ? '<span class="drop-status"></span>'
    : drop.isClaimed
    ? '<span class="drop-status done">получено</span>'
    : drop.canClaim
      ? '<span class="drop-status ready">готово</span>'
      : subs > 0
        ? `<span class="drop-status">${subs} ${subsWord(subs)}</span>`
        : drop.reward
        ? `<span class="drop-status">${drop.requiredMinutes} мин</span>`
        : `<span class="drop-status">${drop.currentMinutes}/${drop.requiredMinutes} мин</span>`;

  const art = drop.benefits?.find((benefit) => benefit.imageUrl)?.imageUrl ?? null;
  const rewardName = drop.benefits?.[0]?.name ?? drop.name;

  // у кампаний-наград прогресс считает Twitch, локально его не видно
  const progress = drop.earned
    ? '<span>награда уже на аккаунте</span>'
    : note
    ? `<span>${esc(note)}</span>`
    : subs > 0
    ? '<span>награда за подписку на канал</span>'
    : drop.reward
    ? '<span>часы считает Twitch — забери награду на twitch.tv/drops/rewards</span>'
    : `<span class="bar ${drop.isClaimed ? 'ok' : ''}" style="width:120px"><i style="width:${percent(drop.progress)}"></i></span>
        <span>${Math.round(clamp01(drop.progress) * 100)}%</span>`;

  return `
  <div class="drop-row">
    <div class="drop-thumb">
      <span class="thumb-fallback">${ICONS.gift}</span>
      ${art ? `<img src="${esc(art)}" alt="" loading="lazy" data-fallback />` : ''}
    </div>
    <div class="drop-main">
      <div class="drop-name" title="${esc(rewardName)}">${esc(drop.name)}</div>
      <div class="drop-sub">${progress}</div>
    </div>
    ${label}
  </div>`;
}

/* ---------------------------------------------------------------- log view */

function badgeRow(badge) {
  const subs = Number(badge.requiredSubs ?? 0);
  const minutes = Number(badge.requiredMinutes ?? 0);
  let condition =
    subs > 0
      ? `${subs} ${subsWord(subs)}`
      : minutes > 0
        ? `${minutes} мин просмотра`
        : 'Приобретите подписку первого уровня или подарочную подписку первого уровня';
  const title = badge.game ? `${badge.name} — ${badge.game}` : badge.name;
  const art = badge.imageUrl;
  const status = badge.claimed
    ? '<span class="drop-status done">получен</span>'
    : badge.paid
      ? `<div style="display:flex;flex-direction:column;align-items:flex-end;gap:4px">
          <button class="paid-link" data-action="open-category" data-slug="${esc(badge.gameSlug)}" data-name="${esc(badge.game ?? badge.campaign ?? '')}" title="Открыть категорию на Twitch">
            <span class="paid-label">Платный</span>
            <span class="paid-sub">открыть категорию</span>
          </button>
          ${badge.webUrl ? `<button class="btn-ghost small" data-action="open-url" data-url="${esc(badge.webUrl)}">Условия</button>` : ''}
        </div>`
      : badge.evolution && !badge.evolutionReady
        ? `<button class="btn-ghost small" disabled title="Сначала нужен ${esc(badge.evolutionRequires)}" style="opacity:.45;cursor:not-allowed">Фармить</button>`
        : badge.available === false
          ? `<button class="btn-ghost small" disabled title="${esc(badge.note || 'Сейчас недоступен')}" style="opacity:.45;cursor:not-allowed">Недоступен</button>`
          : `<button class="btn-ghost small" data-action="farm-game" data-slug="${esc(badge.gameSlug)}">Фармить</button>`;
  return `
  <div class="drop-row">
    <div class="drop-thumb">
      <span class="thumb-fallback">${ICONS.gift}</span>
      ${art ? `<img src="${esc(art)}" alt="" loading="lazy" data-fallback />` : ''}
    </div>
    <div class="drop-main">
      <div class="drop-name" title="${esc(title)}">${esc(title)}</div>
      <div class="drop-sub"><span>${esc(condition)}</span></div>
      ${badge.note ? `<div class="drop-sub" style="opacity:.9"><span>${esc(badge.note)}</span></div>` : ''}
    </div>
    ${status}
  </div>`;
}

function badgesView() {
  const badges = state.snapshot?.badges ?? [];
  const free = badges.filter((badge) => !badge.paid);
  const paid = badges.filter((badge) => badge.paid);
  const group = (title, items) =>
    items.length
      ? `<section class="campaign">
          <div class="campaign-head">
            <div class="campaign-name">${title}</div>
            <div class="campaign-time">${items.filter((badge) => badge.claimed).length}/${items.length}</div>
          </div>
          ${items.map(badgeRow).join('')}
        </section>`
      : '';
  const claimedCount = badges.filter((badge) => badge.claimed).length;
  const freeLeft = free.filter((badge) => badge.available && !badge.claimed).length;
  return `
  <div class="stats-row">
    <div class="stat"><div class="stat-label">Всего</div><div class="stat-value">${badges.length}</div></div>
    <div class="stat"><div class="stat-label">Получено</div><div class="stat-value">${claimedCount}</div></div>
    <div class="stat"><div class="stat-label">Бесплатных осталось</div><div class="stat-value">${freeLeft}</div></div>
  </div>
  ${
    badges.length
      ? group('Бесплатные', free) + group('Платные', paid)
      : '<div class="empty"><h3>Бейджей пока нет</h3><p>Обновите каталог — кампании со значками появятся автоматически.</p></div>'
  }`;
}

function logsView() {
  const logs = [...(state.snapshot.logs ?? [])].reverse();
  if (!logs.length) {
    return '<div class="empty"><h3>Журнал пуст</h3><p>Здесь появятся события: загрузка кампаний, смена стримера, полученные награды.</p></div>';
  }
  const lines = logs
    .map(
      (entry) => `
    <div class="log-line ${esc(entry.level)}">
      <span class="log-time">${new Date(entry.at).toLocaleTimeString('ru-RU')}</span>
      <span class="log-msg">${esc(entry.message)}</span>
    </div>`,
    )
    .join('');
  return `<div class="log-view">${lines}</div>`;
}

/* ----------------------------------------------------------- settings view */

function updateHint() {
  const info = state.update;
  if (!info) return 'Проверяет релизы на GitHub и ставит новую версию сама';
  if (info.error) return `Не удалось проверить: ${info.error}`;
  if (!info.available) return `Установлена последняя версия (${info.current})`;
  return `Доступна ${info.latest}, у вас ${info.current}`;
}

function updateBlock() {
  const info = state.update;
  if (!info?.available) return '';
  const size = info.assetSize ? ` · ${(Number(info.assetSize) / 1048576).toFixed(1)} МБ` : '';
  return `
    <div class="row" style="align-items:flex-start">
      <div>
        <div class="row-label">Что нового в ${esc(info.latest ?? '')}</div>
        <div class="row-hint" style="white-space:pre-wrap">${esc((info.notes ?? '').slice(0, 900))}</div>
      </div>
      <button class="btn-primary" data-action="update-install">Скачать и установить${esc(size)}</button>
    </div>`;
}

function streakSheet() {
  const streak = state.snapshot.streak ?? {};
  const channels = streak.channels ?? [];
  const channel = channels.find((item) => item.login === state.streakChannel);
  if (!channel) return '';
  const sessions = (streak.sessions ?? []).filter((session) => session.login === channel.login);
  const fmtTime = (value) => {
    if (!value) return '';
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    const sameDay = date.toDateString() === new Date().toDateString();
    const time = date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
    return sameDay ? time : date.toLocaleDateString('ru-RU', { day: '2-digit', month: '2-digit' }) + ' ' + time;
  };
  const deltas = sessions
    .map((session) => {
      const before = Number(session.pointsBefore);
      const after = Number(session.pointsAfter);
      return before > 0 && after > 0 ? after - before : null;
    })
    .filter((value) => value !== null && Number.isFinite(value));
  const sessionMinutes = (session) => {
    const startedAt = new Date(session.startedAt).getTime();
    const endedAt = session.endedAt ? new Date(session.endedAt).getTime() : Date.now();
    if (Number.isFinite(startedAt) && Number.isFinite(endedAt) && endedAt > startedAt) {
      return Math.max(1, Math.round((endedAt - startedAt) / 60000));
    }
    return Number(session.minutes) || 0;
  };
  const totalMinutes = sessions.reduce((sum, session) => sum + sessionMinutes(session), 0);
  const totalPoints = deltas.reduce((sum, value) => sum + value, 0);
  const best = deltas.length ? Math.max(...deltas) : null;
  const gainedCount = sessions.filter((session) => session.gained).length;
  const streakValue = Number(channel.streak) > 0 ? channel.streak : null;
  const status = channel.live ? 'В эфире' : channel.watchedToday ? 'Смотрел сегодня' : 'Офлайн';

  const rows = sessions.map((session, index) => {
    const before = Number(session.pointsBefore);
    const after = Number(session.pointsAfter);
    const hasBefore = Number.isFinite(before) && before > 0;
    const hasAfter = Number.isFinite(after) && after > 0;
    const delta = hasBefore && hasAfter ? after - before : null;
    const started = fmtTime(session.startedAt);
    const ended = session.endedAt ? fmtTime(session.endedAt) : 'сейчас';
    const minutes = Number(session.minutes) || 0;
    return `
      <div class="streak-sheet-row" style="animation-delay:${index * 26}ms">
        <div class="streak-sheet-time">${started} → ${ended}</div>
        <div class="streak-sheet-points">
          <span>${hasBefore ? before.toLocaleString('ru-RU') : '—'}</span>
          <span class="arrow">→</span>
          <span>${hasAfter ? after.toLocaleString('ru-RU') : '—'}</span>
          ${delta !== null ? `<b class="${delta > 449 ? 'gain' : delta > 0 ? 'plus' : ''}">${delta > 0 ? '+' : ''}${delta}</b>` : ''}
        </div>
        <div class="streak-sheet-dur">${session.unlimited ? '∞' : minutes + ' мин'}</div>
        ${session.gained ? '<span class="streak-sheet-gain">серия</span>' : ''}
      </div>`;
  }).join('');

  return `
  <div class="streak-sheet-backdrop${state.streakClosing ? '' : ' open'}" data-action="streak-sheet-close"></div>
  <aside class="streak-sheet${state.streakClosing ? ' closing' : state.streakAnimating ? ' opening' : ' open'}" role="dialog" aria-label="Статистика ${esc(channel.login)}">
    <div class="streak-sheet-grab"></div>
    <header class="streak-sheet-head">
      <div class="streak-sheet-avatar">
        ${channel.avatar ? `<img src="${esc(channel.avatar)}" alt="" onerror="this.remove()" />` : esc((channel.login ?? '?').slice(0, 1).toUpperCase())}
      </div>
      <div class="streak-sheet-title">
        <div class="streak-sheet-name">${esc(channel.login)}</div>
        <div class="streak-sheet-status"><span class="streak-dot${channel.live ? '' : ' offline'}"></span>${status}</div>
      </div>
      <div class="streak-sheet-series">
        <b>${streakValue ?? '—'}</b>
        <span>серия</span>
      </div>
      <button class="streak-sheet-close" data-action="streak-sheet-close" title="Закрыть">×</button>
    </header>
    <div class="streak-sheet-metrics">
      <div><b>${sessions.length}</b><span>просмотров</span></div>
      <div><b>${totalMinutes}</b><span>минут</span></div>
      <div><b>${deltas.length ? (totalPoints > 0 ? '+' : '') + totalPoints : '—'}</b><span>баллов</span></div>
      <div><b>${best !== null ? (best > 0 ? '+' : '') + best : '—'}</b><span>лучший прирост</span></div>
      <div><b>${gainedCount}</b><span>продлений</span></div>
    </div>
    <div class="streak-sheet-list">
      ${rows || '<div class="streak-sheet-empty">Просмотров этого канала ещё не было. Запусти просмотр — здесь появятся время, баллы и результат.</div>'}
    </div>
  </aside>`;
}


function closeStreakSheet() {
  state.streakClosing = true;
  state.streakAnimating = false;
  const backdrop = document.querySelector('.streak-sheet-backdrop');
  if (backdrop) backdrop.classList.remove('open');
  render();
  setTimeout(() => {
    state.streakChannel = null;
    state.streakClosing = false;
    document.body.classList.remove('sheet-open');
    render();
  }, 260);
}

function streaksView() {
  const streak = state.snapshot.streak ?? {};
  const channels = (streak.channels ?? []).filter((channel) => channel && channel.login);
  const live = channels.filter((item) => item.live);
  const online = live.length;
  const withStreak = channels.filter((item) => Number(item.streak) > 0).length;
  const watched = channels.filter((item) => item.watchedToday).length;
  const limit = Number(streak.dailyLimit ?? 5);
  const min = Number(streak.minMinutes ?? 6);
  const max = Number(streak.maxMinutes ?? 20);
  const delay = Number(streak.delayMaxMinutes ?? 7);
  const active = streak.active;
  const enabled = Boolean(streak.enabled);
  const maxStreak = channels.reduce((best, item) => Math.max(best, Number(item.streak) || 0), 0);

  const card = (channel, compact) => {
    const streakValue = Number(channel.streak) > 0 ? channel.streak : null;
    const gained = channel.streakGainedAt
      && new Date(channel.streakGainedAt).toDateString() === new Date().toDateString();
    const counted = channel.watchedToday
      && channel.broadcastId
      && channel.broadcastId === channel.lastWatchedBroadcast;
    const status = gained
      ? 'Серия продлена сегодня'
      : counted
        ? 'Этот эфир уже засчитан'
        : channel.live
          ? 'В эфире'
          : channel.watchedToday
            ? 'Смотрел сегодня'
            : 'Офлайн';
    return `
      <div class="streak-card${channel.live ? ' live' : ''}${channel.watchedToday ? ' done' : ''}${compact ? ' compact' : ''}" data-action="streak-card" data-login="${esc(channel.login)}">
        <div class="streak-avatar">
          ${channel.avatar ? `<img src="${esc(channel.avatar)}" alt="" loading="lazy" onerror="this.remove()" />` : esc((channel.login ?? '?').slice(0, 1).toUpperCase())}
        </div>
        <div class="streak-info">
          <div class="streak-name">${esc(channel.login)}${channel.live ? '<span class="streak-dot"></span>' : ''}</div>
          <div class="streak-status">${status}${channel.watchedToday && channel.live ? ' · отмечен' : ''}</div>
        </div>
        <div class="streak-days${streakValue ? '' : ' no-value'}">
          <b>${streakValue ?? '—'}</b>
          <span>серия</span>
        </div>
        ${channel.live && !active
          ? `<button class="streak-watch" data-action="streak-watch" data-login="${esc(channel.login)}" data-id="${esc(channel.id)}" title="Смотреть сейчас">${ICONS.play}</button>`
          : '<span class="streak-watch spacer"></span>'}
      </div>`;
  };

  const liveCards = live.map((channel) => card(channel, false)).join('');
  const otherChannels = channels
    .filter((channel) => !channel.live)
    .sort((a, b) => (Number(b.streak) || 0) - (Number(a.streak) || 0) || String(a.login).localeCompare(String(b.login)));
  const otherCards = otherChannels.map((channel) => card(channel, true)).join('');

  return `
  <div class="streak-hero">
    <div class="streak-hero-main">
      <div class="streak-kicker">Watch Streak Keeper</div>
      <h2>Серии под контролем</h2>
      <p>Приложение заходит на живые каналы из подписок и смотрит их, пока фарм дропов стоит. Заход со случайной задержкой, просмотр ${min}–${max} минут.</p>
      <div class="streak-metrics">
        <div class="streak-metric"><b>${withStreak}</b><span>с серией</span></div>
        <div class="streak-metric"><b>${online}</b><span>в эфире</span></div>
        <div class="streak-metric"><b>${watched}<i>/${limit}</i></b><span>сегодня</span></div>
        <div class="streak-metric"><b>${maxStreak}</b><span>лучшая серия</span></div>
      </div>
    </div>
    <div class="streak-hero-side">
      <div class="streak-power-wrap">
        <button class="streak-power${enabled ? ' on' : ''}" data-action="streak-toggle" aria-pressed="${enabled}">
          <span class="streak-power-ring"></span>
          <span class="streak-power-text">${enabled ? 'Автопилот включён' : 'Автопилот выключен'}</span>
          <span class="streak-power-switch"></span>
        </button>
        <button class="streak-power-more" data-action="streak-menu" title="Запустить просмотр без ограничения">▾</button>
        ${state.streakMenu ? `
          <div class="streak-menu">
            <div class="streak-menu-title">Смотреть без ограничения</div>
            ${live.length
              ? live.map((channel) => `
                <button class="streak-menu-item" data-action="streak-watch" data-login="${esc(channel.login)}" data-id="${esc(channel.id)}" data-minutes="0">
                  <span class="streak-menu-avatar">${channel.avatar ? `<img src="${esc(channel.avatar)}" alt="" />` : esc((channel.login ?? '?').slice(0, 1).toUpperCase())}</span>
                  <span class="streak-menu-login">${esc(channel.login)}</span>
                  <span class="streak-menu-inf">∞</span>
                </button>`).join('')
              : '<div class="streak-menu-empty">Нет каналов в эфире</div>'}
          </div>` : ''}
      </div>
      <button class="streak-stop" data-action="streak-stop" ${active ? '' : 'disabled'}>Стоп</button>
      ${active
        ? `<div class="streak-active"><span class="spinner"></span><span>Смотрю ${esc(active.login)}</span><em>${active.unlimited ? '∞' : `~${active.minutes} мин`}</em></div>`
        : `<div class="streak-idle">${online ? `${online} ${online === 1 ? 'канал' : 'каналов'} в эфире` : 'Никто из подписок не в эфире'}</div>`}
    </div>
  </div>

  <div class="streak-toolbar">
    <div class="streak-limit">
      <span>Каналов в сутки</span>
      <input class="streak-limit-input" type="number" min="1" max="20" value="${limit}" data-action="streak-limit" />
    </div>
    <div class="streak-toolbar-info">задержка до ${delay} мин · ${channels.length} каналов в подписках</div>
    <div class="streak-toolbar-actions">
      <button class="btn-ghost small" data-action="streak-refresh">Обновить список</button>
      <button class="btn-ghost small" data-action="streak-relink">Переподключить Twitch</button>
    </div>
  </div>

  <div class="streak-section">
    <div class="streak-section-head">
      <h3>Сейчас в эфире</h3>
      <span>${online}/${channels.length}</span>
    </div>
    ${liveCards || '<div class="streak-empty">Пока никто из подписок не в эфире. Как только стример начнёт трансляцию, он появится здесь.</div>'}
  </div>

  <div class="streak-section">
    <div class="streak-section-head">
      <h3>Все подписки</h3>
      <span>${channels.length}</span>
    </div>
    <div class="streak-grid">
      ${otherCards || '<div class="streak-empty">Список пуст. Нажми «Обновить список»: для подписок нужен вход через Twitch.</div>'}
    </div>
  </div>
`;
}

function settingsView() {
  const settings = state.snapshot.settings ?? {};
  return `
  <div class="settings-grid">
    <div class="card">
      <h3>Автофарм</h3>
      <p>Как приложение ведёт себя, когда награды уже капают.</p>

      <div class="row">
        <div>
          <div class="row-label">Автоматически забирать награды</div>
          <div class="row-hint">Как только дроп добит до 100%</div>
        </div>
        <button class="switch ${settings.autoClaim ? 'on' : ''}" data-action="toggle-setting" data-key="autoClaim"></button>
      </div>

      <div class="row">
        <div>
          <div class="row-label">Отправлять события просмотра (spade)</div>
          <div class="row-hint">Через эти события Twitch засчитывает минуты просмотра. Выключайте только если Twitch начал ограничивать запросы — тогда прогресс дропов остановится</div>
        </div>
        <button class="switch ${settings.sendSpadeEvents ? 'on' : ''}" data-action="toggle-setting" data-key="sendSpadeEvents"></button>
      </div>

      <div class="row" style="flex-direction:column;align-items:stretch;gap:8px">
        <div>
          <div class="row-label">Возобновлять фарм после перезапуска</div>
          <div class="row-hint">Если приложение закрылось, фарм продолжится сам в этом окне. 0 — не возобновлять</div>
        </div>
        <div style="display:flex;align-items:center;gap:12px">
          <input id="resume-slider" type="range" min="0" max="60" step="1" value="${Number(settings.resumeMinutes ?? 15)}" style="flex:1;accent-color:var(--accent)" />
          <span id="resume-value" style="min-width:64px;text-align:right;font-family:var(--mono);font-size:12px;color:var(--text-2)">${Number(settings.resumeMinutes ?? 15)} мин</span>
        </div>
      </div>
      <div class="row">
        <div>
          <div class="row-label">Сворачивать в трей при закрытии</div>
          <div class="row-hint">Крестик прячет окно, фарм продолжается. Выключите, чтобы крестик завершал приложение</div>
        </div>
        <button class="switch ${settings.minimizeToTray === false ? '' : 'on'}" data-action="toggle-setting" data-key="minimizeToTray"></button>
      </div>

      <div class="row" style="flex-direction:column;align-items:stretch;gap:8px">
        <div>
          <div class="row-label">Обновлять склад кампаний</div>
          <div class="row-hint">Раз в ${settings.refreshMinutes ?? 10} минут</div>
        </div>
        <div style="display:flex;align-items:center;gap:12px">
          <input id="refresh-slider" type="range" min="3" max="60" step="1" value="${Number(settings.refreshMinutes ?? 10)}" style="flex:1;accent-color:var(--accent)" />
          <span id="refresh-value" style="min-width:64px;text-align:right;font-family:var(--mono);font-size:12px;color:var(--text-2)">${Number(settings.refreshMinutes ?? 10)} мин</span>
        </div>
      </div>
    </div>


    <div class="card">
      <h3>Обслуживание</h3>
      <div class="row" style="flex-direction:column;align-items:stretch;gap:10px">
        <div>
          <div class="row-label">Диагностика</div>
          <div class="row-hint">Соберёт версии, состояние сервисов, доступность Twitch и журнал. Без логина, токена и путей с именем пользователя<br>Файл: <code>data/profile/diagnostics/latest.txt</code></div>
        </div>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="btn-ghost" data-action="diagnostics-open">Открыть отчёт</button>
          <button class="btn-ghost" data-action="diagnostics">Собрать отчёт</button>
          <button class="btn-ghost" data-action="copy-logs">Скопировать журнал</button>
        </div>
      </div>
      <div class="row" style="display:block">
        <div class="row-label">Последние события</div>
        <div class="row-hint">Те же записи, что попадают в отчёт диагностики</div>
        <div style="max-height:220px;overflow:auto;margin-top:8px">${logsView()}</div>
      </div>
      <div class="row">
        <div>
          <div class="row-label">Обновления</div>
          <div class="row-hint">${updateHint()}</div>
        </div>
        <button class="btn-ghost" data-action="update-check">Проверить обновления</button>
      </div>
      ${updateBlock()}
    </div>

    <div class="card">
      <h3>О приложении</h3>
      <p>DropForge — открытый аналог автофарма Twitch Drops. Приложение поддерживает сессию просмотра и следит за прогрессом.</p>
      <div style="margin-top:10px">
        <button class="btn-ghost small" data-action="open-url" data-url="https://github.com/CreativeNa/dropforge-tauri">Страница проекта на GitHub</button>
      </div>
      <div class="row">
        <div class="row-label muted">Версия</div>
        <div class="mono muted" id="app-version">${esc(state.appVersion ?? '—')}</div>
      </div>
    </div>
  </div>`;
}

/* ------------------------------------------------------------------ render */

function legacyBanner() {
  const legacy = state.snapshot?.legacy;
  if (!legacy?.stale) return '';
  return `
    <div class="legacy-banner">
      <div>
        <b>Запущена устаревшая копия приложения.</b>
        <span>Установленная версия лежит в <code>${esc(legacy.installExe)}</code></span>
      </div>
      <button class="btn-primary small" data-action="legacy-launch">Запустить установленную</button>
    </div>`;
}

function render() {
  const root = document.getElementById('root');
  if (!root) return;
  if (sliderDragging) {
    sliderPendingRender = true;
    return;
  }

  const searchWasFocused = document.activeElement?.id === 'search';
  const caret = searchWasFocused ? document.activeElement.selectionStart : null;
  const sheetScroll = document.querySelector('.sheet-body')?.scrollTop ?? 0;

  if (!state.snapshot) {
    root.innerHTML = `<div class="auth-screen"><div class="auth-left"><div class="auth-brand"><div class="brand-mark">${ICONS.logo}</div><span class="brand-name">DropForge</span></div><p class="auth-lead">Загрузка…</p></div></div>`;
    return;
  }

  if (!state.snapshot.auth?.loggedIn) {
    root.innerHTML = authScreen();
    return;
  }

  const content =
    state.view === 'drops'
      ? dropsView()
      : state.view === 'badges'
        ? badgesView()
        : state.view === 'streaks'
          ? streaksView()
          : settingsView();

  root.innerHTML = `
    <div class="app">
      ${sidebar()}
      <main class="main">
        ${topbar()}
        ${legacyBanner()}
        <div class="content" id="content">${content}</div>
      </main>
    </div>
    ${state.sheetGame ? sheet() : ''}
    ${state.streakChannel ? streakSheet() : ''}
    ${state.device && state.snapshot.auth?.loggedIn ? deviceOverlay() : ''}`;

  const contentEl = document.getElementById('content');
  if (contentEl) contentEl.scrollTop = state.scrollTop;

  for (const img of document.querySelectorAll('img[data-fallback]')) {
    img.addEventListener('error', () => img.remove(), { once: true });
  }
  if (searchWasFocused) {
    const field = document.getElementById('search');
    if (field) {
      field.focus();
      const position = caret ?? field.value.length;
      field.setSelectionRange(position, position);
    }
  }
  if (state.sheetGame) {
    const sheetBody = document.querySelector('.sheet-body');
    if (sheetBody) sheetBody.scrollTop = sheetScroll;
  }
}

/* ----------------------------------------------------------------- actions */

async function withBusy(key, fn) {
  setBusy(key, true);
  render();
  try {
    return await fn();
  } catch (error) {
    toast(error.message ?? String(error), 'error');
  } finally {
    setBusy(key, false);
    render();
  }
}

let forceLoggedOut = false;

function applySnapshot(snapshot) {
  // код устройства приходит в снимке — событий WebView может не быть
  if (snapshot?.device?.userCode) {
    state.device = snapshot.device;
  } else if (snapshot?.auth?.loggedIn) {
    state.device = null;
  }
  if (!snapshot) return;
  state.snapshot = snapshot;
  if (state.sheetGame && !snapshot.games.some((game) => game.slug === state.sheetGame)) {
    state.sheetGame = null;
  }
  // Окно больше не ждёт сеть при старте, поэтому игра из --game может приехать
  // позже первого рендера: открываем карточку, как только она появилась.
  if (pendingSheetGame && snapshot.games.some((game) => game.slug === pendingSheetGame)) {
    state.sheetGame = pendingSheetGame;
    pendingSheetGame = null;
  }
  render();
  maybeAutoRefresh();
}

const actions = {
  nav(target) {
    state.view = target.dataset.view;
    state.scrollTop = 0;
    render();
  },
  async refresh() {
    state.query = '';
    await withBusy('refresh', async () => {
      const snapshot = await api.refresh();
      applySnapshot(snapshot);
    });
  },
  async start() {
    await withBusy('farm', async () => {
      applySnapshot(await api.start());
    });
  },
  async stop() {
    await withBusy('farm', async () => {
      applySnapshot(await api.stop());
    });
  },
  async 'pin-game'(target) {
    await withBusy(`pin:${target.dataset.slug}`, async () => {
      applySnapshot(await api.togglePin(target.dataset.slug));
    });
  },
  async 'toggle-game'(target) {
    await withBusy(`game:${target.dataset.slug}`, async () => {
      applySnapshot(await api.toggleGame(target.dataset.slug));
    });
  },
  async 'farm-game'(target) {
    await withBusy('farm', async () => {
      applySnapshot(await api.farmGame(target.dataset.slug));
      toast('Фарм запущен', 'success');
    });
  },
  'clear-search'() {
    state.query = '';
    state.page = 0;
    render();
  },
  'page'(target) {
    state.page = Number(target.dataset.page) || 0;
    state.scrollTop = 0;
    render();
  },
  'drops-tab'(target) {
    state.dropsTab = target.dataset.tab === 'local' ? 'local' : 'global';
    state.query = '';
    state.page = 0;
    Promise.resolve(api.saveSettings?.({ dropsTab: state.dropsTab })).catch(() => {});
    render();
  },
  async 'local-check'(form) {
    if (typeof api.localCheckChannel !== 'function') {
      toast('Локальные дропсы доступны в Tauri-сборке', 'error');
      return;
    }
    const raw = form.querySelector('input[name="login"]')?.value?.trim() ?? '';
    const login = raw
      .replace(/^https?:\/\/(www\.)?twitch\.tv\//i, '')
      .split(/[\/?#]/)[0]
      .trim();
    if (!login) return;
    await withBusy('local', async () => {
      const snapshot = await api.localCheckChannel(login);
      if (snapshot?.error) {
        toast(snapshot.error, 'error');
        return;
      }
      applySnapshot(snapshot);
      const check = snapshot.settings?.localCheck;
      toast(check?.found ? `Найдено кампаний: ${check.found}` : 'Локальных дропсов сейчас нет', check?.found ? 'success' : 'error');
    });
  },
  async 'local-clear'() {
    if (typeof api.localClearChannel !== 'function') return;
    await withBusy('local', async () => {
      applySnapshot(await api.localClearChannel());
    });
  },
  async 'legacy-launch'() {
    if (typeof api.legacyLaunch !== 'function') return;
    await api.legacyLaunch();
  },

  async 'select-farmable'() {
    await withBusy('farm', async () => {
      const slugs = state.snapshot.games.filter((game) => game.canEarn).map((game) => game.slug);
      if (!slugs.length) {
        toast('Нет игр с доступными дропами', 'error');
        return;
      }
      applySnapshot(await api.selectOnly(slugs));
    });
  },
  async 'clear-queue'() {
    const count = state.snapshot.settings?.games?.length ?? 0;
    if (!count) return;
    if (count > 1 && !window.confirm(`Убрать все игры из очереди (${count})?`)) return;
    await withBusy('queue', async () => {
      applySnapshot(await api.clearQueue());
      toast('Очередь очищена');
    });
  },
  'open-game'(target) {
    state.sheetGame = target.dataset.slug;
    render();
  },
  'close-sheet'() {
    state.sheetGame = null;
    render();
  },
  async 'claim-all'() {
    await withBusy('claim', async () => {
      const { claimed, snapshot } = await api.claimAll();
      applySnapshot(snapshot);
      toast(claimed ? `Получено наград: ${claimed}` : 'Пока нечего забирать', claimed ? 'success' : 'info');
    });
  },
  async 'login-browser'() {
    state.error = null;
    render();
    await api.loginWithBrowser();
    toast('Завершите вход в открывшемся окне Twitch');
  },
  async 'login-device'() {
    state.error = null;
    state.device = null;
    render();
    toast('Открываю браузер — подтвердите вход на Twitch');
    try {
      const snapshot = await api.loginWithDevice();
      state.device = null;
      applySnapshot(snapshot);
      toast(`Вы вошли как ${snapshot.auth?.user?.login ?? 'аккаунт Twitch'}`, 'success');
    } catch (error) {
      const message = error?.message ?? String(error);
      state.device = null;
      if (/отмен/i.test(message)) {
        render();
        toast('Вход отменён');
      } else {
        state.error = message;
        render();
      }
    }
  },
  async 'cancel-device'() {
    state.device = null;
    render();
    await api.cancelDevice().catch(() => {});
  },
  async 'open-activation'() {
    const uri = state.device?.uri ?? 'https://www.twitch.tv/activate';
    await api.openExternal(uri);
  },
  /** «Кампания-награда» (например, GTA VI): забирается на странице наград Twitch. */
  async 'open-category'(target) {
    const slug = target.dataset.slug ?? '';
    const name = target.dataset.name ?? '';
    const url =
      slug && !slug.startsWith('local:')
        ? `https://www.twitch.tv/directory/category/${encodeURIComponent(slug)}`
        : `https://www.twitch.tv/search?term=${encodeURIComponent(name)}`;
    await api.openExternal(url);
  },
  async diagnostics() {
    state.busy.add('diagnostics');
    render();
    try {
      const report = await api.diagnostics();
      state.diagnostics = report;
      const opened = await api.diagnosticsOpen();
      toast(opened?.ok ? 'Отчёт открыт в проводнике и скопирован в буфер' : 'Отчёт сохранён: ' + (report?.path ?? 'data/profile/diagnostics/latest.txt'));
    } catch (error) {
      toast('Диагностика: ' + error.message, 'error');
    } finally {
      state.busy.delete('diagnostics');
      render();
    }
  },

  async 'copy-logs'() {
    const logs = [...(state.snapshot?.logs ?? [])].reverse();
    if (!logs.length) { toast('Журнал пуст', 'error'); return; }
    if (typeof api.copyText !== 'function') { toast('Копирование недоступно', 'error'); return; }
    const text = logs
      .map((entry) => `${new Date(entry.at).toISOString()} [${entry.level}] ${entry.message}`)
      .join('\n');
    const result = await api.copyText(text);
    toast(result?.ok ? 'Журнал скопирован' : 'Не удалось скопировать', result?.ok ? 'info' : 'error');
  },

  async 'logout'() {
    if (typeof api.logout !== 'function') return;
    forceLoggedOut = true;
    try {
      applySnapshot(await api.logout());
    } finally {
      forceLoggedOut = false;
    }
    state.view = 'drops';
    state.sheetGame = null;
    render();
  },

  'streak-card'(target) {
    state.streakChannel = target.dataset.login;
    state.streakClosing = false;
    state.streakAnimating = true;
    document.body.classList.add('sheet-open');
    render();
    setTimeout(() => {
      state.streakAnimating = false;
    }, 340);
  },

  'streak-sheet-close'() {
    closeStreakSheet();
  },

  async 'streak-menu'() {
    state.streakMenu = !state.streakMenu;
    render();
  },

  async 'streak-stop'() {
    try {
      await api.streakStop();
      toast('Просмотр остановлен', 'info');
    } catch (error) {
      toast('Стрики: ' + error.message, 'error');
    }
  },

  async 'streak-toggle'() {
    const enabled = !(state.snapshot.streak?.enabled);
    applySnapshot({ ...state.snapshot, streak: { ...(state.snapshot.streak ?? {}), enabled } });
    try {
      await api.streakSave({ enabled });
    } catch (error) {
      toast('Стрики: ' + error.message, 'error');
    }
  },

  async 'streak-relink'() {
    try {
      toast('Подтверди вход в браузере — профиль сохранится', 'info');
      const snapshot = await api.loginWithDevice();
      if (snapshot?.ready) applySnapshot(snapshot);
      toast('Twitch переподключён', 'success');
      const result = await api.streakRefresh();
      if (result?.error) toast(result.error, 'error');
    } catch (error) {
      toast('Переподключение: ' + error.message, 'error');
    }
  },

  async 'streak-refresh'() {
    try {
      const result = await api.streakRefresh();
      if (result?.error) toast(result.error, 'error');
      else toast('Подписки обновлены: ' + result.count, 'info');
    } catch (error) {
      toast('Стрики: ' + error.message, 'error');
    }
  },

  async 'streak-watch'(target) {
    try {
      const minutes = target.dataset.minutes !== undefined ? Number(target.dataset.minutes) : undefined;
      const result = await api.streakWatch(target.dataset.login, target.dataset.id, minutes);
      if (result?.error) toast(result.error, 'error');
      else {
        state.streakMenu = false;
        toast('Смотрю ' + target.dataset.login, 'info');
      }
    } catch (error) {
      toast('Стрики: ' + error.message, 'error');
    }
  },

  async 'copy-summary'() {
    try {
      const result = await api.reportSummary();
      toast(result?.copied ? 'Краткая сводка скопирована' : 'Не удалось скопировать', result?.copied ? 'info' : 'error');
    } catch (error) {
      toast('Сводка: ' + error.message, 'error');
    }
  },

  async 'report-issue'() {
    const field = document.getElementById('problem-text');
    const comment = field?.value ?? '';
    try {
      const result = await api.reportIssue(comment);
      if (result?.ok) {
        if (field) field.value = '';
        toast('Отчёт сохранён: ' + result.path + ' · сводка в буфере');
      } else {
        toast('Отчёт: ' + (result?.error ?? 'не удалось'), 'error');
      }
    } catch (error) {
      toast('Отчёт: ' + error.message, 'error');
    }
  },

  async 'gta-start-day'() {
    const now = new Date().toISOString();
    const snapshot = await api.saveSettings({ gtaDay: { startedAt: now, watched: 0, completedAt: null } });
    if (snapshot?.ready) applySnapshot(snapshot);
    render();
    // приоритет: Bratishkinoff до 2 декабря, иначе обычный автофарм категории
    const local = await api.farmGame('local:bratishkinoff');
    let resultSnap = local;
    if (!local?.status?.running) {
      const fallback = await api.farmGame('grand-theft-auto-vi');
      if (fallback?.ready) resultSnap = fallback;
    }
    if (resultSnap?.ready) applySnapshot(resultSnap);
    render();
  },

  async 'close-day'() {
    const collection = (state.snapshot?.collections ?? []).find((item) => item.chains?.length);
    const slug = collection?.gameSlug ?? 'grand-theft-auto-vi';
    const today = new Date().toISOString().slice(0, 10);
    const closed = { ...(state.snapshot?.settings?.closedDays ?? {}) };
    closed[slug] = today;
    delete closed['undefined'];
    const snapshot = await api.saveSettings({ closedDays: closed });
    if (snapshot?.ready) applySnapshot(snapshot);
    render();
  },

  async 'gta-chains'(target) {
    const delta = Number(target?.dataset?.delta ?? 0);
    const current = (state.snapshot?.collections ?? []).find((item) => item.chains?.length);
    const value = Math.max(0, Math.min(6, Number(current?.chests ?? 0) + delta));
    const snapshot = await api.saveSettings({ gtaChains: value });
    if (snapshot?.ready) applySnapshot(snapshot);
    render();
  },

  async 'open-url'(target) {
    const url = target?.dataset?.url;
    if (url) await api.openExternal(url);
  },

  async 'diagnostics-open'() {
    try {
      const result = await api.diagnosticsOpen();
      if (!result?.ok) toast(result?.error ?? 'Отчёт ещё не собран', 'error');
      else toast('Отчёт открыт, текст скопирован в буфер');
    } catch (error) {
      toast('Диагностика: ' + error.message, 'error');
    }
  },

  async 'update-check'() {
    state.busy.add('update-check');
    render();
    try {
      const info = await api.updateCheck();
      state.update = info;
      if (info?.error) toast('Обновления: ' + info.error, 'error');
      else if (info?.available) toast('Доступна версия ' + info.latest);
      else toast('Установлена последняя версия');
    } catch (error) {
      toast('Обновления: ' + error.message, 'error');
    } finally {
      state.busy.delete('update-check');
      render();
    }
  },

  async 'update-install'() {
    const url = state.update?.assetUrl;
    if (!url) { toast('В релизе нет контрольной суммы — автообновление отключено, скачайте установщик вручную', 'error'); return; }
    toast('Скачиваю обновление… окно закроется само');
    try {
      const result = await api.updateInstall(url, state.update?.assetDigest);
      if (!result?.started) toast('Обновление: ' + (result?.error ?? 'не удалось'), 'error');
    } catch (error) {
      toast('Обновление: ' + error.message, 'error');
    }
  },

  async 'open-rewards'() {
    const game = state.snapshot?.games?.find((item) => item.slug === state.sheetGame);
    const campaign = state.snapshot?.campaigns?.find(
      (item) => item.reward && (!game || item.game.slug === game.slug),
    );
    await api.openExternal(campaign?.webUrl ?? 'https://www.twitch.tv/drops/rewards');
  },
  async 'login-token'(target) {
    const input = target.querySelector('#token');
    const token = input?.value?.trim();
    if (!token) {
      toast('Вставьте OAuth-токен', 'error');
      return;
    }
    state.error = null;
    await withBusy('login', async () => {
      try {
        const snapshot = await api.loginWithToken(token);
        applySnapshot(snapshot);
        toast('Вход выполнен', 'success');
      } catch (error) {
        state.error = error.message;
        render();
      }
    });
  },
  async logout() {
    const snapshot = await api.logout();
    applySnapshot(snapshot);
    state.sheetGame = null;
  },
  async 'toggle-setting'(target) {
    const key = target.dataset.key;
    const value = !state.snapshot.settings?.[key];
    applySnapshot(await api.saveSettings({ [key]: value }));
  },
  async 'open-data'() {
    await api.openDataDir();
  },
};

document.addEventListener('click', (event) => {
  const target = event.target.closest('[data-action]');
  if (!target) return;
  const action = target.dataset.action;
  if (target.tagName === 'FORM') return;
  event.preventDefault();
  event.stopPropagation();
  const handler = actions[action];
  if (!handler) {
    console.warn(`[ui] нет обработчика для действия "${action}"`);
    return;
  }
  Promise.resolve(handler(target)).catch((error) => toast(error.message, 'error'));
});

document.addEventListener('submit', (event) => {
  const form = event.target.closest('form[data-action]');
  if (!form) return;
  event.preventDefault();
  const handler = actions[form.dataset.action];
  if (handler) Promise.resolve(handler(form)).catch((error) => toast(error.message, 'error'));
});

document.addEventListener('change', (event) => {
  const target = event.target;
  if (target.dataset.action === 'streak-limit') {
    const value = Math.max(1, Math.min(20, Number(target.value) || 5));
    if (state.snapshot.streak) state.snapshot.streak.dailyLimit = value;
    api.streakSave({ dailyLimit: value }).catch(() => {});
    return;
  }
  const sliders = {
    'resume-slider': { key: 'resumeMinutes', min: 0, max: 60 },
    'refresh-slider': { key: 'refreshMinutes', min: 3, max: 60 },
  };
  const config = sliders[target.id];
  if (!config) return;
  const value = Math.max(config.min, Math.min(config.max, Number(target.value) || config.min));
  if (state.snapshot?.settings) state.snapshot.settings[config.key] = value;
  Promise.resolve(api.saveSettings({ [config.key]: value })).catch(() => {});
});
document.addEventListener('input', (event) => {
  const target = event.target;
  if (target.id === 'resume-slider' || target.id === 'refresh-slider') {
    const label = document.getElementById(target.id === 'resume-slider' ? 'resume-value' : 'refresh-value');
    if (label) label.textContent = target.value + ' мин';
    const key = target.id === 'resume-slider' ? 'resumeMinutes' : 'refreshMinutes';
    if (state.snapshot?.settings) state.snapshot.settings[key] = Number(target.value) || 0;
    return;
  }
  if (target.id === 'search') {
    state.query = target.value;
    state.page = 0;
    clearTimeout(searchTimer);
    // перерисовываем с задержкой и возвращаем фокус с позицией каретки,
    // иначе поле теряет ввод и сетка прыгает на каждый символ
    searchTimer = setTimeout(() => {
      const caret = target.selectionStart ?? target.value.length;
      render();
      const next = document.getElementById('search');
      if (next) {
        next.focus();
        try { next.setSelectionRange(caret, caret); } catch (error) {}
      }
    }, 140);
    return;
    const query = state.query.trim().toLowerCase();
    const section = document.getElementById('games-section');
    if (!section) {
      render();
      return;
    }

    // Только показываем/скрываем готовые карточки — без пересоздания DOM,
    // иначе картинки перезагружаются и сетка дёргается.
    let visible = 0;
    for (const slot of section.querySelectorAll('.game-card')) {
      const match = !query || (slot.dataset.name ?? '').includes(query);
      slot.hidden = !match;
      if (match) visible += 1;
    }

    const hint = document.getElementById('games-hint');
    if (hint) {
      hint.textContent = `${visible} ${visible === 1 ? 'игра' : 'игр'} · нажмите на карточку, чтобы увидеть награды`;
    }
    const grid = document.getElementById('games-grid');
    const empty = document.getElementById('games-empty');
    if (grid) grid.hidden = visible === 0;
    if (empty) empty.hidden = visible > 0;
    return;
  }

});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && state.streakChannel) {
    closeStreakSheet();
    return;
  }
  if (event.key === 'Escape' && state.sheetGame) {
    state.sheetGame = null;
    render();
  }
});

document.addEventListener('scroll', (event) => {
  if (event.target?.id === 'content') state.scrollTop = event.target.scrollTop;
}, true);

/* -------------------------------------------------------------------- boot */

async function boot() {
  const params = new URLSearchParams(location.search);

  if (api?.version) {
    api
      .version()
      .then((info) => {
        state.appVersion = info?.app ?? null;
        if (state.view === 'settings') render();
      })
      .catch(() => {});
  }

  const applyDeepLink = () => {
    const view = params.get('view');
    if (view && ['drops', 'logs', 'settings'].includes(view)) state.view = view;
    const game = params.get('game');
    if (!game) return;
    if (state.snapshot?.games?.some((item) => item.slug === game)) state.sheetGame = game;
    else pendingSheetGame = game;
  };

  if (!api) {
    state.snapshot = demoSnapshot();
    state.demo = true;
    applyDeepLink();
    render();
    return;
  }

  if (params.get('demo') === '1') {
    state.snapshot = demoSnapshot();
    state.demo = true;
    applyDeepLink();
    render();
    return;
  }

  try {
    const initial = await api.init();
    state.snapshot = initial;
    state.demo = Boolean(initial.demo);
    state.dropsTab = initial.settings?.dropsTab === 'local' ? 'local' : 'global';
    applyDeepLink();
    if (params.get('preview') === 'code') {
      state.device = {
        userCode: 'GTWMDPNH',
        uri: 'https://www.twitch.tv/activate?device-code=GTWMDPNH',
      };
    }
    render();
    maybeAutoRefresh();
  } catch (error) {
    toast(`Ошибка запуска: ${error.message}`, 'error');
    state.snapshot = demoSnapshot();
    render();
  }

  for (const channel of api.events) {
    api.on(channel, (payload) => {
      switch (channel) {
        case 'state':
          applySnapshot(payload);
          break;
        case 'status':
          if (state.snapshot) {
            state.snapshot.status = payload;
            const dot = document.querySelector('.farm-card');
            if (dot) render();
          }
          break;
        case 'log':
          if (state.snapshot) {
            state.snapshot.logs = [...(state.snapshot.logs ?? []), payload].slice(-200);
            if (state.view === 'logs') render();
          }
          break;
        case 'refresh':
          if (payload.active) {
            refreshStartedAt = Date.now();
            setBusy('refresh', true);
            if (state.view === 'drops') render();
          } else {
            const left = 900 - (Date.now() - refreshStartedAt);
            setTimeout(() => {
              setBusy('refresh', false);
              if (state.view === 'drops') render();
            }, Math.max(0, left));
          }
          break;
        case 'auth':
          if (state.snapshot) state.snapshot.auth = payload;
          render();
          break;
        case 'auth-error':
          state.error = payload;
          render();
          break;
        case 'auth-cancelled':
          toast('Вход отменён');
          break;
        case 'device-code':
          state.device = payload;
          render();
          break;
        case 'progress': {
          if (!state.snapshot) break;
          const campaign = state.snapshot.campaigns.find((item) => item.id === payload.campaignId);
          const drop = campaign?.drops.find((item) => item.id === payload.dropId);
          if (drop) {
            drop.currentMinutes = payload.currentMinutes;
            drop.progress = clamp01(payload.currentMinutes / (payload.requiredMinutes || 1));
            if (state.view === 'drops') render();
          }
          break;
        }
        default:
          break;
      }
    });
  }
}

boot();
// timing probe

