const { app, BrowserWindow, ipcMain, shell, session, Tray, Menu, nativeImage, safeStorage } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const { writeFile } = require('node:fs/promises');
const { RustPubSub } = require('./pubsub-process.cjs');

const root = path.resolve(__dirname, '..', '..');

const isScreenshot = process.argv.includes('--screenshot');
const isDemo = process.argv.includes('--demo');

// Chromium в некоторых окружениях (в частности в этом рабочем каталоге)
// не может поднять AppContainer-песочницу и падает с STATUS_BREAKPOINT.
// Приложение показывает только свой локальный интерфейс, поэтому запуск
// без песочницы безопасен. Вернуть её: DROPFORGE_KEEP_SANDBOX=1.
if (process.env.DROPFORGE_KEEP_SANDBOX !== '1') {
  app.commandLine.appendSwitch('no-sandbox');
}

// Пасс-ключи Twitch не нужны: их системное окно «Выберите ключ доступа»
// перекрывает приложение при открытии страницы входа.
app.commandLine.appendSwitch('disable-features', 'WebAuthenticationConditionalUI');

app.disableHardwareAcceleration();
app.setAppUserModelId('dev.dropforge.app');

/**
 * Портативный профиль: если рядом с приложением можно писать, держим кэш
 * Chromium и сессию Twitch там, а не в %APPDATA%. Это и переносимость,
 * и обход окружений, где запись в профиль пользователя запрещена.
 */
function setupPortableProfile() {
  // В собранном приложении ресурсы лежат в app.asar (только чтение),
  // поэтому портативные данные кладём рядом с DropForge.exe.
  const portableRoot = app.isPackaged ? path.dirname(app.getPath('exe')) : root;
  const portable = path.join(portableRoot, 'data');
  try {
    fs.mkdirSync(path.join(portable, 'session'), { recursive: true });
    const probe = path.join(portable, '.write-test');
    fs.writeFileSync(probe, 'ok');
    fs.unlinkSync(probe);
  } catch {
    return false;
  }
  app.setPath('userData', path.join(portable, 'profile'));
  app.setPath('sessionData', path.join(portable, 'session'));
  try {
    fs.mkdirSync(app.getPath('userData'), { recursive: true });
  } catch {
    /* создастся позже */
  }
  return true;
}

const portableProfile = setupPortableProfile();

let mainWindow = null;
let engine = null;
let authWindow = null;
let EngineClass = null;
let deviceLoginController = null;
let tray = null;
let rustPubSub = null;
let isQuitting = false;
let trayHintShown = false;

function resolvePubSubExe() {
  const candidates = [];
  if (app.isPackaged) {
    candidates.push(path.join(process.resourcesPath, 'native', 'pubsub-svc', 'dropforge-pubsub-svc.exe'));
  }
  candidates.push(path.join(root, 'rust', 'pubsub-svc', 'target', 'release', 'dropforge-pubsub-svc.exe'));
  return candidates.find((candidate) => fs.existsSync(candidate)) ?? null;
}

/** PubSub переезжает в Rust-процесс; JS-клиент остаётся фолбэком. */
function resolveWatcherExe() {
  const candidates = [];
  if (app.isPackaged) {
    candidates.push(path.join(process.resourcesPath, 'native', 'watcher-svc', 'dropforge-watcher-svc.exe'));
  }
  candidates.push(path.join(root, 'rust', 'watcher-svc', 'target', 'release', 'dropforge-watcher-svc.exe'));
  return candidates.find((candidate) => fs.existsSync(candidate)) ?? null;
}

function startRustPubSub() {
  const exePath = resolvePubSubExe();
  if (!engine?.session?.token || !exePath) return false;
  rustPubSub?.stop();
  rustPubSub = new RustPubSub({
    exePath,
    token: engine.session.token,
    userId: engine.session.userId,
  });
  engine.pubsub.stop();
  rustPubSub.on('message', ({ topic, payload }) => engine.applyPubSubMessage(topic, payload));
  rustPubSub.on('status', ({ connected }) => {
    engine.rustPubsubConnected = Boolean(connected);
    broadcast('state', engine.snapshot());
  });
  rustPubSub.on('error', (message) => engine.log(`PubSub (Rust): ${message}`, 'warn'));
  rustPubSub.start();
  engine.log('PubSub: подключён Rust-сервис');
  return true;
}

function stopRustPubSub() {
  rustPubSub?.stop();
  rustPubSub = null;
  if (engine) engine.rustPubsubConnected = false;
}

function broadcast(channel, payload) {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send(channel, payload);
  }
}

async function createEngine() {
  if (!EngineClass) {
    ({ Engine: EngineClass } = await import('../core/engine.js'));
  }
  // Токен на диске шифруется средствами ОС (DPAPI на Windows).
  let tokenCipher = null;
  try {
    if (safeStorage.isEncryptionAvailable()) {
      tokenCipher = {
        encrypt: (text) => safeStorage.encryptString(text).toString('base64'),
        decrypt: (value) => {
          try {
            return safeStorage.decryptString(Buffer.from(value, 'base64'));
          } catch {
            return null;
          }
        },
      };
    }
  } catch {
    tokenCipher = null;
  }

  engine = new EngineClass({
    dataDir: app.getPath('userData'),
    onLog: () => {},
    tokenCipher,
    rustWatchExe: resolveWatcherExe(),
  });
  engine.on('state', (state) => broadcast('state', state));
  engine.on('log', (entry) => broadcast('log', entry));
  engine.on('status', (status) => broadcast('status', status));
  engine.on('refresh', (event) => broadcast('refresh', event));
  engine.on('auth', (auth) => broadcast('auth', auth));
  engine.on('progress', (event) => broadcast('progress', event));
  engine.on('pubsub', (event) => broadcast('pubsub', event));
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1320,
    height: 860,
    minWidth: 1080,
    minHeight: 700,
    backgroundColor: '#0a0c11',
    show: false,
    autoHideMenuBar: true,
    title: 'DropForge',
    icon: path.join(root, 'src', 'ui', 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      spellcheck: false,
    },
  });

  const argValue = (prefix) => {
    const found = process.argv.find((arg) => arg.startsWith(`${prefix}=`));
    return found ? found.slice(prefix.length + 1) : null;
  };
  const query = {};
  if (isDemo) query.demo = '1';
  if (argValue('--view')) query.view = argValue('--view');
  if (argValue('--game')) query.game = argValue('--game');
  if (argValue('--preview')) query.preview = argValue('--preview');

  mainWindow.loadFile(path.join(root, 'src', 'ui', 'index.html'), { query });

  mainWindow.once('ready-to-show', () => mainWindow.show());
  mainWindow.on('close', (event) => {
    if (isQuitting) return;
    if (engine?.settings?.minimizeToTray === false) return;
    event.preventDefault();
    mainWindow.hide();
    if (tray && !trayHintShown) {
      trayHintShown = true;
      try {
        tray.displayBalloon({
          title: 'DropForge',
          content: 'Свёрнуто в трей. Фарм продолжается, выход — через меню иконки в трее.',
        });
      } catch {
        /* balloons поддерживаются не везде */
      }
    }
  });
  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  mainWindow.webContents.on('render-process-gone', (_event, details) => {
    console.error('renderer gone:', JSON.stringify(details));
    if (isScreenshot) app.exit(2);
  });
  mainWindow.webContents.on('did-fail-load', (_event, code, description) => {
    console.error('did-fail-load', code, description);
  });
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: 'deny' };
  });

  if (isScreenshot) {
    mainWindow.webContents.once('did-finish-load', async () => {
      await new Promise((resolve) => setTimeout(resolve, 4000));
      try {
        const image = await mainWindow.webContents.capturePage();
        const suffix = [
          isDemo ? 'demo' : 'auth',
          query.view ? `view-${query.view}` : '',
          query.game ? `game-${query.game}` : '',
          query.preview ? `preview-${query.preview}` : '',
        ]
          .filter(Boolean)
          .join('-');
        const target = path.join(root, 'shots', `shot-${suffix}.png`);
        await writeFile(target, image.toPNG());
        console.log(`screenshot saved: ${target}`);
      } catch (error) {
        console.error('screenshot failed', error);
      }
      app.exit(0);
    });
  }
}

function showWindow() {
  if (!mainWindow || mainWindow.isDestroyed()) {
    createWindow();
    return;
  }
  if (mainWindow.isMinimized()) mainWindow.restore();
  mainWindow.show();
  mainWindow.focus();
}

function createTray() {
  if (tray) return;
  // читаем через fs: путь внутри app.asar иначе может не открыться
  let image = nativeImage.createEmpty();
  try {
    image = nativeImage.createFromBuffer(fs.readFileSync(path.join(root, 'src', 'ui', 'icon.png')));
  } catch (error) {
    console.error('tray icon:', error.message);
  }
  if (!image.isEmpty()) image = image.resize({ width: 16, height: 16 });

  tray = new Tray(image);
  tray.setToolTip('DropForge — автофарм Twitch Drops');
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: 'Открыть DropForge', click: showWindow },
      {
        label: 'Запустить фарм',
        click: () => {
          showWindow();
          engine?.startFarming().catch(() => {});
        },
      },
      {
        label: 'Остановить фарм',
        click: () => {
          engine?.stopFarming().catch(() => {});
        },
      },
      { type: 'separator' },
      {
        label: 'Выход',
        click: () => {
          isQuitting = true;
          app.quit();
        },
      },
    ]),
  );
  tray.on('click', showWindow);
  tray.on('double-click', showWindow);
}

async function findAuthToken(partitionSession) {
  const cookies = await partitionSession.cookies.get({ name: 'auth-token' });
  const cookie = cookies.find((item) => item.domain && item.domain.includes('twitch.tv'));
  return cookie ? cookie.value : null;
}

/** Вход через встроенное окно Twitch: забираем cookie auth-token. */
async function loginWithBrowser() {
  if (authWindow && !authWindow.isDestroyed()) {
    authWindow.focus();
    return { pending: true };
  }

  const partition = 'persist:twitch-login';
  authWindow = new BrowserWindow({
    width: 520,
    height: 740,
    parent: mainWindow || undefined,
    title: 'Вход в Twitch',
    autoHideMenuBar: true,
    backgroundColor: '#0a0c11',
    webPreferences: {
      partition,
      // preload должен работать в мире страницы, чтобы убрать WebAuthn
      // до того, как Twitch вызовет системное окно пасс-ключа
      contextIsolation: false,
      nodeIntegration: false,
      sandbox: true,
      preload: path.join(__dirname, 'no-webauthn.cjs'),
    },
  });

  const twitchSession = session.fromPartition(partition);
  let finished = false;

  const poll = setInterval(async () => {
    if (finished || !authWindow || authWindow.isDestroyed()) return;
    try {
      const token = await findAuthToken(twitchSession);
      if (!token) return;
      finished = true;
      clearInterval(poll);
      authWindow.close();
      authWindow = null;
      try {
        await engine.loginWithToken(token);
        await engine.refresh({ silent: true }).catch(() => {});
        broadcast('state', engine.snapshot());
      } catch (error) {
        broadcast('auth-error', error.message);
      }
    } catch {
      /* cookie может быть недоступна в момент записи — просто пробуем снова */
    }
  }, 1200);

  authWindow.on('closed', () => {
    clearInterval(poll);
    authWindow = null;
    if (!finished) broadcast('auth-cancelled', {});
  });

  await authWindow.loadURL('https://www.twitch.tv/login');
  return { pending: true };
}

function registerIpc() {
  ipcMain.handle('app:init', async () => {
    await engine.init().catch((error) => {
      console.error('init failed', error);
    });
    // С сохранённой сессией сразу подтягиваем кампании, но окно не ждёт сеть:
    // обновление идёт в фоне, а результат прилетит событием 'state'.
    if (engine.loggedIn) {
      startRustPubSub();
      engine.refresh({ silent: true }).catch((error) => {
        console.error('initial refresh failed', error.message);
        engine.log(`Не удалось обновить кампании: ${error.message}`, 'error');
        broadcast('state', engine.snapshot());
      });
    }
    const snapshot = engine.snapshot();
    return { ...snapshot, demo: isDemo, platform: process.platform };
  });

  ipcMain.handle('app:snapshot', async () => engine.snapshot());
  ipcMain.handle('app:version', async () => ({ app: app.getVersion(), electron: process.versions.electron }));

  ipcMain.handle('auth:login-token', async (_event, token) => {
    // loginWithToken сам обновляет склад кампаний
    await engine.loginWithToken(token);
    startRustPubSub();
    return engine.snapshot();
  });
  ipcMain.handle('auth:login-device', async () => {
    const controller = new AbortController();
    deviceLoginController?.abort();
    deviceLoginController = controller;
    try {
      await engine.loginWithDeviceCode(
        ({ userCode, uri }) => {
          broadcast('device-code', { userCode, uri });
          shell.openExternal(uri).catch(() => {});
        },
        { signal: controller.signal },
      );
      return engine.snapshot();
    } finally {
      if (deviceLoginController === controller) deviceLoginController = null;
    }
  });
  ipcMain.handle('auth:cancel-device', async () => {
    deviceLoginController?.abort();
    return { cancelled: true };
  });
  ipcMain.handle('auth:login-browser', async () => loginWithBrowser());
  ipcMain.handle('auth:logout', async () => {
    stopRustPubSub();
    await engine.logout();
    return engine.snapshot();
  });

  ipcMain.handle('inventory:refresh', async () => {
    await engine.refresh();
    return engine.snapshot();
  });
  ipcMain.handle('farm:start', async () => {
    await engine.startFarming();
    return engine.snapshot();
  });
  ipcMain.handle('farm:clear', async () => {
    await engine.clearQueue();
    return engine.snapshot();
  });
  ipcMain.handle('farm:game', async (_event, slug) => {
    await engine.farmGame(slug);
    return engine.snapshot();
  });
  ipcMain.handle('farm:stop', async () => {
    await engine.stopFarming();
    return engine.snapshot();
  });
  ipcMain.handle('farm:skip', async () => {
    engine.watcher.skipCurrent();
    return engine.snapshot();
  });
  ipcMain.handle('games:pin', async (_event, slug) => {
    await engine.togglePin(slug);
    return engine.snapshot();
  });
  ipcMain.handle('games:toggle', async (_event, slug) => {
    await engine.toggleGame(slug);
    return engine.snapshot();
  });
  ipcMain.handle('games:select-only', async (_event, slugs) => {
    await engine.selectOnly(slugs);
    return engine.snapshot();
  });
  ipcMain.handle('drops:claim-all', async () => {
    const claimed = await engine.claimAll();
    return { claimed, snapshot: engine.snapshot() };
  });
  ipcMain.handle('settings:save', async (_event, patch) => {
    await engine.saveSettings(patch);
    return engine.snapshot();
  });
  ipcMain.handle('shell:open', async (_event, url) => {
    if (typeof url === 'string' && /^https?:\/\//i.test(url)) await shell.openExternal(url);
    return true;
  });
  ipcMain.handle('app:data-dir', async () => {
    const dir = app.getPath('userData');
    await shell.openPath(dir);
    return dir;
  });
  ipcMain.handle('app:quit', async () => app.quit());
}

app.on('web-contents-created', (_event, contents) => {
  contents.session.setPermissionRequestHandler((_webContents, _permission, callback) => callback(false));
  contents.on('will-navigate', (event, url) => {
    const allowed = url.startsWith('file://') || /^https:\/\/([\w-]+\.)*twitch\.tv\//i.test(url);
    if (!allowed) event.preventDefault();
  });
});

app.on('before-quit', () => {
  isQuitting = true;
  stopRustPubSub();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

app.whenReady().then(async () => {
  console.log(`[main] ready, electron ${process.versions.electron}`);
  await createEngine();
  console.log('[main] engine ok');
  registerIpc();
  createTray();
  createWindow();
  console.log('[main] window created');

  // Режим разработки: правки в src/ui перезагружают окно без перезапуска.
  if (process.argv.includes('--dev')) {
    const uiDir = path.join(root, 'src', 'ui');
    let reloadTimer = null;
    try {
      fs.watch(uiDir, { recursive: true }, () => {
        clearTimeout(reloadTimer);
        reloadTimer = setTimeout(() => {
          if (mainWindow && !mainWindow.isDestroyed()) {
            mainWindow.webContents.reloadIgnoringCache();
            console.log('[dev] окно перезагружено');
          }
        }, 120);
      });
      console.log('[dev] слежу за src/ui');
    } catch (error) {
      console.error('[dev] не удалось следить за src/ui:', error.message);
    }
  }
}).catch((error) => {
  console.error('[main] startup failed', error);
});

// Единственный экземпляр: повторный запуск фокусирует уже открытое окно.
const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
}
