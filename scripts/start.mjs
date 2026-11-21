#!/usr/bin/env node
/**
 * Запуск приложения в отдельном процессе.
 *
 * 1. Обёртка npm-пакета electron (`electron .`) спавнит бинарник из Node с
 *    унаследованным stdio — в ограниченных окружениях Windows (AppContainer,
 *    запрет именованных каналов) такой запуск падает с STATUS_BREAKPOINT.
 *    Поэтому стартуем Electron напрямую.
 * 2. В некоторых окружениях Chromium не поднимает песочницу рендерера и
 *    завершается сразу. Тогда повторяем запуск с --no-sandbox: приложение
 *    показывает только локальный интерфейс, так что это безопасно.
 */
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const isWindows = process.platform === 'win32';
const binary = path.join(root, 'node_modules', 'electron', 'dist', isWindows ? 'electron.exe' : 'electron');

if (!existsSync(binary)) {
  console.error('Electron не установлен. Выполните: npm install');
  process.exit(1);
}

const userArgs = process.argv.slice(2);

function launch(extra = []) {
  const child = spawn(binary, [root, ...userArgs, ...extra], {
    detached: true,
    stdio: 'ignore',
    cwd: root,
  });
  child.unref();
  return child;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const first = launch();
let exited = false;
let exitCode = null;
first.once('exit', (code, signal) => {
  exited = true;
  exitCode = signal ?? code;
});

await sleep(2500);

if (!exited) {
  console.log(`DropForge запущен (pid ${first.pid})`);
  process.exit(0);
}

console.log(`Electron завершился при старте (${exitCode}), повторяю без песочницы…`);
const second = launch(['--no-sandbox']);
await sleep(2500);

if (second.exitCode === null) {
  console.log(`DropForge запущен без песочницы (pid ${second.pid})`);
  process.exit(0);
}

console.error('Не удалось запустить Electron. Проверьте вывод: npm start -- --enable-logging');
process.exit(1);
