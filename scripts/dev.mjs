#!/usr/bin/env node
/**
 * Режим разработки: приложение из исходников + автоперезагрузка.
 *
 * - изменения в src/ui перезагружают окно (это делает сам main.cjs с флагом --dev);
 * - изменения в src/main и src/core перезапускают Electron.
 *
 *   npm run dev
 */
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const isWindows = process.platform === 'win32';
const binary = path.join(root, 'node_modules', 'electron', 'dist', isWindows ? 'electron.exe' : 'electron');

if (!fs.existsSync(binary)) {
  console.error('Electron не установлен. Выполните: npm install');
  process.exit(1);
}

const userArgs = process.argv.slice(2);
let child = null;
let restarting = false;
let timer = null;

function start() {
  // без песочницы: в ограниченных окружениях Windows иначе падает на старте
  child = spawn(binary, [root, '--dev', '--no-sandbox', ...userArgs], { stdio: 'inherit', cwd: root });
  child.on('exit', (code) => {
    if (restarting) return;
    process.exit(code ?? 0);
  });
}

function restart(reason) {
  if (restarting) return;
  restarting = true;
  console.log(`\n[dev] перезапуск: ${reason}`);
  const previous = child;
  child = null;
  const done = () => {
    restarting = false;
    start();
  };
  if (!previous || previous.exitCode !== null) {
    done();
    return;
  }
  previous.once('exit', done);
  previous.kill();
}

start();
console.log('[dev] следим за src/main и src/core; окно перезагружается само');

for (const dir of ['src/main', 'src/core']) {
  const full = path.join(root, dir);
  fs.watch(full, { recursive: true }, (_event, filename) => {
    if (filename && !/\.(js|cjs|mjs|json)$/.test(String(filename))) return;
    clearTimeout(timer);
    timer = setTimeout(() => restart(`${dir}/${filename ?? ''}`), 250);
  });
}
