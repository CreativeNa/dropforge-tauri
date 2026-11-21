#!/usr/bin/env node
/**
 * Сборка портативного Tauri-дистрибутива:
 *   dist-tauri/DropForge/DropForge.exe + сервисы рядом
 *
 *   npm run build:tauri
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const out = path.join(root, 'dist-tauri', 'DropForge');

const app = path.join(root, 'rust', 'tauri-app', 'target', 'release', 'dropforge-tauri.exe');
const services = [
  ['pubsub-svc', path.join(root, 'rust', 'pubsub-svc', 'target', 'release', 'dropforge-pubsub-svc.exe')],
  ['watcher-svc', path.join(root, 'rust', 'watcher-svc', 'target', 'release', 'dropforge-watcher-svc.exe')],
];

for (const [name, file] of [['tauri', app], ...services]) {
  if (!fs.existsSync(file)) {
    console.error(`Нет ${file}. Сначала: cargo build --release --manifest-path rust/${name === 'tauri' ? 'tauri-app' : name}/Cargo.toml`);
    process.exit(1);
  }
}

fs.rmSync(path.join(root, 'dist-tauri'), { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
fs.copyFileSync(app, path.join(out, 'DropForge.exe'));
for (const [name, file] of services) {
  const target = path.join(out, name);
  fs.mkdirSync(target, { recursive: true });
  fs.copyFileSync(file, path.join(target, path.basename(file)));
}

fs.copyFileSync(path.join(root, 'install.ps1'), path.join(out, 'install.ps1'));

fs.writeFileSync(
  path.join(out, 'README.txt'),
  [
    'DropForge (Tauri)',
    '',
    'Портативная версия: настройки, токен и кэш кампаний лежат в data/ рядом с DropForge.exe.',
    'WebView2 берётся из системы (Windows 10/11).',
    '',
    'Установка: powershell -ExecutionPolicy Bypass -File install.ps1 (копирует в %LOCALAPPDATA% и делает ярлык).',
    'Либо просто запускайте DropForge.exe из этой папки.',
    '',
    'Сервисы pubsub-svc и watcher-svc запускаются приложением автоматически.',
    'Вход: вставьте OAuth-токен Twitch в приложении (кнопка «Вход по токену»).',
    '',
  ].join('\n'),
  'utf8',
);

const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const zipName = `DropForge-${version}-win64.zip`;
const zipPath = path.join(root, 'dist-tauri', zipName);
const packed = spawnSync('tar.exe', ['-a', '-c', '-f', zipPath, '-C', path.join(root, 'dist-tauri'), 'DropForge'], {
  stdio: 'inherit',
});
if (packed.status !== 0) {
  console.error('Не удалось упаковать архив');
  process.exit(packed.status ?? 1);
}
const size = (fs.statSync(zipPath).size / (1024 * 1024)).toFixed(1);
console.log(`Готово: ${zipPath} (${size} МБ)`);
