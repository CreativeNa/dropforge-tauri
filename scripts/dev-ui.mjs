#!/usr/bin/env node
/**
 * UI без пересборки: локальный статический сервер для src/ui + `tauri dev`
 * с devUrl. Правки CSS/JS подхватываются по F5 в окне, бинарь не трогаем.
 *
 *   npm run dev:ui
 *
 * Профиль берётся у установленного приложения (DROPFORGE_DATA), чтобы
 * не логиниться заново.
 */
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const uiDir = path.join(root, 'src', 'ui');
const port = Number(process.env.DROPFORGE_UI_PORT ?? 5173);
const installData = path.join(process.env.LOCALAPPDATA ?? '', 'Programs', 'DropForge', 'data');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.json': 'application/json; charset=utf-8',
};

// предыдущий dev-запуск мог оставить процессы WebView2, которые держат
// пользовательский каталог и дают "resource busy" при создании окна
if (process.platform === 'win32') {
  // установленное приложение держит single-instance мьютекс — закрываем его,
  // иначе dev-окно откажется стартовать
  spawnSync('taskkill', ['/IM', 'DropForge.exe', '/F'], { stdio: 'ignore' });
  spawnSync('taskkill', ['/IM', 'msedgewebview2.exe', '/F'], { stdio: 'ignore' });
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1500);
}

const server = http.createServer((request, response) => {
  const url = new URL(request.url ?? '/', `http://127.0.0.1:${port}`);
  const relative = url.pathname === '/' ? 'index.html' : decodeURIComponent(url.pathname).replace(/^\/+/, '');
  const file = path.join(uiDir, relative);
  if (!file.startsWith(uiDir) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    response.writeHead(404).end('not found');
    return;
  }
  response.writeHead(200, {
    'Content-Type': MIME[path.extname(file).toLowerCase()] ?? 'application/octet-stream',
    'Cache-Control': 'no-store',
  });
  fs.createReadStream(file).pipe(response);
});

server.listen(port, '127.0.0.1', () => {
  console.log(`[dev:ui] UI на http://127.0.0.1:${port}`);
  // отдельный identifier: у dev-сборки свой каталог WebView2, иначе окно
  // конфликтует с установленным приложением ("resource busy")
  const config = JSON.stringify({
    identifier: 'dev.dropforge.app.dev',
    build: { devUrl: `http://127.0.0.1:${port}`, frontendDist: '../../src/ui' },
  });
  const cliArgs = ['--yes', '@tauri-apps/cli@2', 'dev', '--config', config];
  const child = spawn(
    process.platform === 'win32' ? 'cmd' : 'npx',
    process.platform === 'win32' ? ['/c', 'npx', ...cliArgs] : cliArgs,
    {
      cwd: path.join(root, 'rust', 'tauri-app'),
      stdio: 'inherit',
      env: {
        ...process.env,
        DROPFORGE_DATA: installData,
        // цель сборки вне рабочего каталога: иначе WebView2 не может создать
        // профиль внутри каталога с ограниченными ACL
        CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? path.join(process.env.TEMP ?? '.', 'dropforge-dev-target'),
      },
    },
  );
  child.on('exit', (code) => {
    server.close();
    process.exit(code ?? 0);
  });
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    server.close();
    process.exit(0);
  });
}
