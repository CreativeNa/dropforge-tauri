/**
 * Мост между интерфейсом и Tauri-командами.
 *
 * Подключается только когда страница открыта в Tauri (есть window.__TAURI__).
 */
(() => {
  const tauri = window.__TAURI__;
  if (!tauri) return;

  const invoke = tauri.core?.invoke ?? tauri.invoke;
  const call = (command, payload) => {
    if (!invoke) return Promise.reject(new Error('Tauri invoke недоступен'));
    return payload === undefined ? invoke(command) : invoke(command, payload);
  };

  const listeners = new Map();
  const emit = (channel, payload) => {
    for (const handler of listeners.get(channel) ?? []) handler(payload);
  };

  window.dropforge = {
    init: () => call('app_init'),
    snapshot: () => call('app_snapshot'),
    version: () => call('app_version'),
    loginWithToken: (token) => call('auth_login_token', { token }),
    loginWithDevice: () => call('auth_login_device'),
    cancelDevice: () => call('noop'),
    loginWithBrowser: () => call('auth_login_browser'),
    logout: () => call('auth_logout'),
    refresh: () => call('inventory_refresh'),
    start: () => call('farm_start'),
    farmGame: (slug) => call('farm_game', { slug }),
    clearQueue: () => call('farm_clear'),
    stop: () => call('farm_stop'),
    skip: () => call('farm_skip'),
    toggleGame: (slug) => call('games_toggle', { slug }),
    togglePin: (slug) => call('games_pin', { slug }),
    selectOnly: (slugs) => call('games_select_only', { slugs }),
    claimAll: () => call('drops_claim_all'),
    saveSettings: (patch) => call('settings_save', { patch }),
    streakSave: (patch) => call('streak_settings_save', { patch }),
    streakRefresh: () => call('streak_refresh'),
    streakWatch: (login, channelId, minutes) => call('streak_watch', { login, channelId, minutes }),
    streakStop: () => call('streak_stop'),
    logsOpen: () => call('logs_open'),
    appLanguage: () => call('app_language'),
    localCheckChannel: (login) => call('local_check_channel', { login }),
    localClearChannel: () => call('local_clear_channel'),
    openExternal: (url) => call('shell_open', { url }),
    openDataDir: () => call('app_data_dir'),
    diagnostics: () => call('diagnostics_report'),
    diagnosticsOpen: () => call('diagnostics_open'),
    reportSummary: () => call('report_summary'),
    reportIssue: (comment) => call('report_issue', { comment }),
  copyText: (text) => call('clipboard_copy_text', { text }),
    updateCheck: () => call('update_check'),
  legacyLaunch: () => call('legacy_launch'),
    updateInstall: (url, digest) => call('update_install', { url, digest }),
    quit: () => call('app_quit'),
    on: (channel, callback) => {
      // polling: слушатели хранятся здесь, а снимок опрашивается ниже
      const handlers = listeners.get(channel) ?? [];
      handlers.push(callback);
      listeners.set(channel, handlers);
      return () => {
        const current = listeners.get(channel) ?? [];
        listeners.set(channel, current.filter((item) => item !== callback));
      };
    },
    events: [
      'state',
      'log',
      'status',
      'refresh',
      'auth',
      'auth-error',
      'auth-cancelled',
      'device-code',
      'progress',
      'pubsub',
    ],
  };
  // каталог кампаний приходит один раз, дальше — только дельта прогресса
  let cachedCampaigns = [];
  let lastStamp = "";
  const applyDelta = (snapshot) => {
    if (!snapshot) return snapshot;
    if (Array.isArray(snapshot.campaigns) && snapshot.campaigns.length) {
      cachedCampaigns = snapshot.campaigns;
      return snapshot;
    }
    if (snapshot.progressDrops && cachedCampaigns.length) {
      const drops = snapshot.progressDrops;
      snapshot.campaigns = cachedCampaigns.map((campaign) => ({
        ...campaign,
        drops: (campaign.drops ?? []).map((drop) => {
          const state = drops[drop.id];
          if (!state) return drop;
          const current = Number(state.currentMinutes) || 0;
          const required = Number(drop.requiredMinutes) || 0;
          const claimed = Boolean(state.isClaimed);
          return {
            ...drop,
            currentMinutes: current,
            remainingMinutes: Math.max(0, required - current),
            progress: required > 0 ? Math.min(1, current / required) : 0,
            isClaimed: claimed,
          };
        }),
      }));
    }
    return snapshot;
  };
  // события бэкенда: без них спиннер «Обновить» не узнаёт, что загрузка идёт
  const tauriEvent = tauri.event;
  if (tauriEvent?.listen) {
    for (const channel of ['log', 'status', 'refresh', 'device-code', 'auth', 'auth-error', 'auth-cancelled', 'progress', 'pubsub']) {
      tauriEvent.listen(channel, (event) => emit(channel, event?.payload ?? event)).catch(() => {});
    }
  }

  let timer = null;
  const pull = async () => {
    try {
      const info = await invoke("app_snapshot_stamp");
      const stamp = info?.stamp ?? "";
      // пока каталог не пришёл ни разу, запрашиваем полный снимок: иначе бэкенд
      // решит, что клиент уже знает кампании, и пришлёт только дельту прогресса
      const needFull = cachedCampaigns.length === 0;
      if (stamp && (stamp !== lastStamp || needFull)) {
        lastStamp = stamp;
        const snapshot = applyDelta(await invoke("app_snapshot", needFull ? {} : { since: lastStamp }));
        emit("state", snapshot);
      }
    } catch {
      /* приложение закрывается */
    }
  };
  const schedule = () => {
    const delay = document.hidden ? 30000 : 5000;
    if (timer) clearTimeout(timer);
    timer = setTimeout(async () => {
      await pull();
      schedule();
    }, delay);
  };
  document.addEventListener("visibilitychange", schedule);
  schedule();

})();
