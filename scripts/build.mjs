#!/usr/bin/env node
/**
 * Сборка portable-версии для Windows x64: dist/DropForge-win32-x64/DropForge.exe
 *
 *   npm run build
 *
 * Папка data/ (профиль Chromium, сессия Twitch, настройки) переносится
 * поверх пересборки, чтобы обновление приложения не разлогинивало пользователя.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { packager } from '@electron/packager';


const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const appDir = path.join(root, 'dist', 'DropForge-win32-x64');
const dataDir = path.join(appDir, 'data');
const backupDir = path.join(root, 'dist', '.data-backup');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));

console.log('Собираю DropForge…');

let saved = false;
if (fs.existsSync(dataDir)) {
  fs.rmSync(backupDir, { recursive: true, force: true });
  fs.cpSync(dataDir, backupDir, { recursive: true });
  saved = true;
  console.log('Профиль сохранён, после сборки верну на место');
}

const appPaths = await packager({
  dir: root,
  name: 'DropForge',
  platform: 'win32',
  arch: 'x64',
  out: path.join(root, 'dist'),
  overwrite: true,
  icon: path.join(root, 'build', 'icon.ico'),
  appVersion: pkg.version,
  ignore: [/^\/(shots|test|dist|data|build|rust)/],
});

if (saved) {
  fs.rmSync(dataDir, { recursive: true, force: true });
  fs.cpSync(backupDir, dataDir, { recursive: true });
  fs.rmSync(backupDir, { recursive: true, force: true });
  console.log('Профиль восстановлен');
}

// napi-модули не грузятся из asar: кладём их рядом с app.asar
const nativeDir = path.join(appDir, 'resources', 'native');
fs.rmSync(nativeDir, { recursive: true, force: true });
for (const crate of ['model-rs2', 'catalog-rs', 'gql-rs', 'http-rs', 'inventory-rs', 'watcher-rs']) {
  const from = path.join(root, 'rust', crate);
  if (!fs.existsSync(from)) continue;
  const to = path.join(nativeDir, crate);
  fs.mkdirSync(to, { recursive: true });
  for (const file of fs.readdirSync(from)) {
    if (file.endsWith('.node') || file === 'index.cjs') {
      fs.copyFileSync(path.join(from, file), path.join(to, file));
    }
  }
}

const watcherExe = path.join(root, 'rust', 'watcher-svc', 'target', 'release', 'dropforge-watcher-svc.exe');
if (fs.existsSync(watcherExe)) {
  const target = path.join(nativeDir, 'watcher-svc');
  fs.mkdirSync(target, { recursive: true });
  fs.copyFileSync(watcherExe, path.join(target, 'dropforge-watcher-svc.exe'));
}

const pubsubExe = path.join(root, 'rust', 'pubsub-svc', 'target', 'release', 'dropforge-pubsub-svc.exe');
if (fs.existsSync(pubsubExe)) {
  const target = path.join(nativeDir, 'pubsub-svc');
  fs.mkdirSync(target, { recursive: true });
  fs.copyFileSync(pubsubExe, path.join(target, 'dropforge-pubsub-svc.exe'));
}

console.log(`Готово: ${appPaths.join(', ')}`);
console.log('Ярлык: npm run shortcut');
