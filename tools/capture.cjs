// Захват GraphQL-запросов страницы drops/campaigns + снимок текста страницы.
// Запуск: node_modules\electron\dist\electron.exe tools --no-sandbox [--profile=name]
const { app, BrowserWindow, session } = require('electron');
const fs = require('node:fs');
const path = require('node:path');

app.commandLine.appendSwitch('no-sandbox');
app.disableHardwareAcceleration();

const profileArg = process.argv.find((arg) => arg.startsWith('--profile='));
const profileName = profileArg ? profileArg.split('=')[1] : 'capture';

const profileDir = path.join(__dirname, '..', 'data', `${profileName}-profile`);
fs.mkdirSync(profileDir, { recursive: true });
app.setPath('userData', profileDir);
app.setPath('sessionData', profileDir);

const logFile = path.join(__dirname, '..', 'cap.log');
const log = (...parts) => fs.appendFileSync(logFile, parts.map(String).join(' ') + '\n');

const captured = [];

app.whenReady().then(async () => {
  log('ready, profile:', profileName);
  const partition = `persist:${profileName}`;
  const ses = session.fromPartition(partition);

  ses.webRequest.onBeforeRequest({ urls: ['https://gql.twitch.tv/*'] }, (details, callback) => {
    try {
      const chunks = details.uploadData ?? [];
      const body = Buffer.concat(chunks.map((part) => part.bytes ?? Buffer.alloc(0))).toString('utf8');
      const parsed = JSON.parse(body);
      for (const op of Array.isArray(parsed) ? parsed : [parsed]) {
        captured.push({ operationName: op.operationName, hash: op.extensions?.persistedQuery?.sha256Hash ?? null, variables: op.variables ?? null });
        log('OP', op.operationName, op.extensions?.persistedQuery?.sha256Hash ?? '—');
      }
    } catch {
      /* не JSON */
    }
    callback({ cancel: false });
  });

  const win = new BrowserWindow({
    width: 1400,
    height: 1000,
    show: false,
    webPreferences: { partition, contextIsolation: true, nodeIntegration: false },
  });

  win.loadURL('https://www.twitch.tv/drops/campaigns').catch((error) => log('load error', error.message));

  await new Promise((r) => setTimeout(r, 12000));

  // Пытаемся согласиться с cookie-баннером и закрыть модалки.
  const clicked = await win.webContents
    .executeJavaScript(`(() => {
      const texts = ['Accept', 'Принять', 'Принять все', '同意', 'Allow all'];
      let clicked = 0;
      for (const button of document.querySelectorAll('button')) {
        const label = (button.textContent || '').trim();
        if (texts.some((t) => label.toLowerCase() === t.toLowerCase()) || /accept all/i.test(label)) {
          button.click();
          clicked += 1;
        }
      }
      return clicked;
    })()`)
    .catch((error) => `js error: ${error.message}`);
  log('нажато кнопок согласия:', clicked);

  await new Promise((r) => setTimeout(r, 15000));

  const info = await win.webContents
    .executeJavaScript(`({ title: document.title, url: location.href, text: (document.body.innerText || '').slice(0, 3000) })`)
    .catch((error) => ({ error: error.message }));
  log('URL:', info.url ?? info.error);
  log('TEXT:\n' + (info.text ?? ''));

  const image = await win.webContents.capturePage().catch(() => null);
  if (image) fs.writeFileSync(path.join(__dirname, '..', 'shots', 'capture-page.png'), image.toPNG());

  const seen = new Set();
  const unique = captured.filter((item) => {
    const key = `${item.operationName}:${item.hash}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  fs.writeFileSync(path.join(__dirname, '..', '_capture.json'), JSON.stringify(unique, null, 2));
  log('уникальных операций:', unique.length);
  for (const item of unique) log(' *', item.operationName, item.hash ?? '—');
  app.exit(0);
});

process.on('uncaughtException', (error) => {
  log('uncaught', error.message);
  app.exit(2);
});
