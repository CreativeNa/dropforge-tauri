#!/usr/bin/env node
/**
 * Headless-проверка флоу после фикса: вход, канал, один тик watcher, прогресс.
 * Окно не открывается, подтверждение в браузере не требуется.
 *
 *   npm run check
 *
 * Код выхода 0 — всё зелёное, 1 — есть провал (печатается, что именно).
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const installDir = path.join(process.env.LOCALAPPDATA ?? '', 'Programs', 'DropForge');
const exe = path.join(installDir, 'DropForge.exe');
const profileDir = path.join(installDir, 'data', 'profile');
const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;
delete env.DROPFORGE_FARM;
delete env.DROPFORGE_TICKS;

const checks = [];
const record = (name, ok, detail) => {
  checks.push({ name, ok, detail });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ` — ${detail}` : ''}`);
};

if (!fs.existsSync(exe)) {
  console.error(`Не найден ${exe}. Установите приложение: dist-tauri\\DropForge\\install.ps1`);
  process.exit(1);
}

// 1. приложение не должно занимать single-instance lock
for (const name of ['DropForge', 'dropforge-watcher-svc', 'dropforge-pubsub-svc']) {
  spawnSync('taskkill', ['/IM', `${name}.exe`, '/F'], { stdio: 'ignore' });
}

// 2. вход: снимок состояния
const snapshotFile = path.join(installDir, 'data', 'snapshot.json');
fs.rmSync(snapshotFile, { force: true });
const dump = spawnSync(exe, [], { cwd: installDir, env: { ...env, DROPFORGE_DUMP: '1' }, encoding: 'utf8', timeout: 60_000 });
if (!fs.existsSync(snapshotFile)) {
  record('снимок состояния', false, `не создан: ${(dump.stdout || dump.stderr || '').trim().slice(0, 200)}`);
} else {
  const snapshot = JSON.parse(fs.readFileSync(snapshotFile, 'utf8'));
  record('вход в Twitch', Boolean(snapshot.auth?.loggedIn), snapshot.auth?.user?.login ?? 'нет логина');
  record('кампании загружены', Number(snapshot.stats?.campaigns) > 0, `кампаний: ${snapshot.stats?.campaigns ?? 0}`);
}

// 3. фарм: один тик watcher (интервал 10 с, лимит 1 тик)
const settingsFile = path.join(profileDir, 'settings.json');
let slug = 'escape-from-tarkov';
try {
  const settings = JSON.parse(fs.readFileSync(settingsFile, 'utf8').replace(/^\uFEFF/, ''));
  slug = settings.games?.[0] ?? slug;
} catch {
  /* настройки могут отсутствовать — берём игру по умолчанию */
}

const farm = spawnSync(exe, [], {
  cwd: installDir,
  env: { ...env, DROPFORGE_FARM: slug, DROPFORGE_TICKS: '1', DROPFORGE_TICK_INTERVAL: '10' },
  encoding: 'utf8',
  timeout: 120_000,
});

const lines = `${farm.stdout ?? ''}`.split(/\r?\n/);
const tickLine = lines.find((line) => line.startsWith('tick: '));
const farmLine = lines.find((line) => line.startsWith('farm: '));

if (!tickLine) {
  record('тик watcher', false, `нет тика. farm: ${farmLine ?? '-'} stderr: ${(farm.stderr ?? '').trim().slice(0, 160)}`);
} else {
  let tick = null;
  try {
    tick = JSON.parse(tickLine.slice('tick: '.length));
  } catch {
    record('разбор тика', false, tickLine.slice(0, 160));
  }
  if (tick) {
    record('сегменты стрима', Number(tick.chunksOk) > 0, `chunksOk: ${tick.chunksOk}/${tick.chunks}`);
    record('spade принят', [200, 204].includes(Number(tick.spadeStatus)), `status: ${tick.spadeStatus}`);
    const progress = tick.progress;
    record('drop-сессия активна', Boolean(progress?.dropID), progress?.dropID ? `dropID: ${progress.dropID}` : 'progress = null');
    if (progress?.dropID) {
      record('минуты засчитаны', Number.isFinite(Number(progress.currentMinutesWatched)), `currentMinutesWatched: ${progress.currentMinutesWatched}`);
    }
  }
}

// 4. прогресс попал в файл
const progressFile = path.join(profileDir, 'progress.json');
record('progress.json обновлён', fs.existsSync(progressFile), fs.existsSync(progressFile) ? '' : 'файла нет');

const failed = checks.filter((item) => !item.ok);
console.log('');
console.log(failed.length ? `ИТОГ: провалов ${failed.length} из ${checks.length}` : `ИТОГ: все ${checks.length} проверок зелёные`);
process.exit(failed.length ? 1 : 0);
