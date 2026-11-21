/**
 * Мост для Tauri: повторяет контракт preload-моста Electron.
 *
 * Подключается только когда страница открыта в Tauri (есть window.__TAURI__),
 * поэтому тот же index.html продолжает работать в Electron.
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
    localCheckChannel: (login) => call('local_check_channel', { login }),
    localClearChannel: () => call('local_clear_channel'),
    openExternal: (url) => call('shell_open', { url }),
    openDataDir: () => call('app_data_dir'),
    diagnostics: () => call('diagnostics_report'),
    diagnosticsOpen: () => call('diagnostics_open'),
    updateCheck: () => call('update_check'),
    updateInstall: (url) => call('update_install', { url }),
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
  // раз в 5 секунд забираем снимок и рассылаем его слушателям
  let lastSnapshot = "";
  setInterval(async () => {
    try {
      const snapshot = await invoke("app_snapshot");
      const key = JSON.stringify(snapshot);
      if (key !== lastSnapshot) {
        lastSnapshot = key;
        emit("state", snapshot);
      }
    } catch {
      /* приложение закрывается */
    }
  }, 5000);
})();
