const { contextBridge, ipcRenderer } = require('electron');

const listeners = new Map();

function on(channel, callback) {
  const handler = (_event, payload) => callback(payload);
  ipcRenderer.on(channel, handler);
  listeners.set(callback, { channel, handler });
  return () => {
    ipcRenderer.removeListener(channel, handler);
    listeners.delete(callback);
  };
}

contextBridge.exposeInMainWorld('dropforge', {
  init: () => ipcRenderer.invoke('app:init'),
  snapshot: () => ipcRenderer.invoke('app:snapshot'),
  version: () => ipcRenderer.invoke('app:version'),
  loginWithToken: (token) => ipcRenderer.invoke('auth:login-token', token),
  loginWithDevice: () => ipcRenderer.invoke('auth:login-device'),
  cancelDevice: () => ipcRenderer.invoke('auth:cancel-device'),
  loginWithBrowser: () => ipcRenderer.invoke('auth:login-browser'),
  logout: () => ipcRenderer.invoke('auth:logout'),
  refresh: () => ipcRenderer.invoke('inventory:refresh'),
  start: () => ipcRenderer.invoke('farm:start'),
  farmGame: (slug) => ipcRenderer.invoke('farm:game', slug),
  clearQueue: () => ipcRenderer.invoke('farm:clear'),
  stop: () => ipcRenderer.invoke('farm:stop'),
  skip: () => ipcRenderer.invoke('farm:skip'),
  toggleGame: (slug) => ipcRenderer.invoke('games:toggle', slug),
  togglePin: (slug) => ipcRenderer.invoke('games:pin', slug),
  selectOnly: (slugs) => ipcRenderer.invoke('games:select-only', slugs),
  claimAll: () => ipcRenderer.invoke('drops:claim-all'),
  saveSettings: (patch) => ipcRenderer.invoke('settings:save', patch),
  openExternal: (url) => ipcRenderer.invoke('shell:open', url),
  openDataDir: () => ipcRenderer.invoke('app:data-dir'),
  quit: () => ipcRenderer.invoke('app:quit'),
  on,
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
});
