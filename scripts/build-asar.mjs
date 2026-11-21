#!/usr/bin/env node
/**
 * Быстрая пересборка уже собранного приложения.
 *
 * Полный `npm run build` каждый раз копирует ~350 МБ и занимает десятки секунд.
 * Здесь в готовый dist подменяется только resources/app.asar, собранный из
 * исходников во временной папке — обычно 1–3 секунды.
 *
 *   npm run build:fast
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createPackage } from '@electron/asar';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const distApp = path.join(root, 'dist', 'DropForge-win32-x64');
const resources = path.join(distApp, 'resources');
const stage = path.join(root, 'dist', '.asar-stage');

if (!fs.existsSync(path.join(distApp, 'DropForge.exe'))) {
  console.error('Сначала выполните полную сборку: npm run build');
  process.exit(1);
}

const started = Date.now();
fs.rmSync(stage, { recursive: true, force: true });
fs.mkdirSync(stage, { recursive: true });

const entries = ['src', 'package.json', 'cli.js', 'LICENSE', 'README.md'];
for (const entry of entries) {
  const from = path.join(root, entry);
  if (!fs.existsSync(from)) continue;
  fs.cpSync(from, path.join(stage, entry), { recursive: true });
}

const target = path.join(resources, 'app.asar');
const tmp = path.join(resources, 'app.asar.tmp');
await createPackage(stage, tmp);
fs.rmSync(target, { force: true });
fs.renameSync(tmp, target);
fs.rmSync(stage, { recursive: true, force: true });

const size = Math.round(fs.statSync(target).size / 1024);
console.log(`app.asar обновлён за ${((Date.now() - started) / 1000).toFixed(1)} с (${size} КБ)`);
console.log('Запуск: dist\\DropForge-win32-x64\\DropForge.exe');
