#!/usr/bin/env node
/**
 * Быстрый цикл правки: собрать только изменённый слой, положить в
 * установленную папку и перезапустить приложение.
 *
 *   node scripts/fast.mjs svc    # watcher-svc + pubsub-svc (секунды)
 *   node scripts/fast.mjs app    # Tauri-шелл, release (около 1.5 минуты)
 *   node scripts/fast.mjs ui     # Tauri-шелл в debug (быстрее release)
 *   node scripts/fast.mjs smoke  # снимок состояния без окна
 *
 * Флаги: --no-restart — не перезапускать приложение.
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const installDir = path.join(process.env.LOCALAPPDATA ?? '', 'Programs', 'DropForge');
const args = process.argv.slice(2);
const target = args.find((item) => !item.startsWith('--')) ?? 'svc';
const restart = !args.includes('--no-restart');

const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;
env.CARGO_TERM_COLOR = 'never';

const log = (message) => console.log(`[fast] ${message}`);

function run(command, commandArgs, options = {}) {
  const started = Date.now();
  const result = spawnSync(command, commandArgs, {
    cwd: root,
    env,
    stdio: options.quiet ? ['ignore', 'pipe', 'pipe'] : 'inherit',
    encoding: 'utf8',
  });
  return { ok: result.status === 0, ms: Date.now() - started, result };
}

function cargo(manifest, profile = 'release') {
  const argsForCargo = ['build', '--manifest-path', manifest];
  if (profile === 'release') argsForCargo.push('--release');
  const { ok, ms } = run('cargo', argsForCargo);
  return { ok, ms };
}

function copy(from, to) {
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
}

function killApp() {
  for (const name of ['DropForge', 'dropforge-watcher-svc', 'dropforge-pubsub-svc']) {
    spawnSync('taskkill', ['/IM', `${name}.exe`, '/F'], { stdio: 'ignore' });
  }
}

function startApp() {
  const exe = path.join(installDir, 'DropForge.exe');
  if (!fs.existsSync(exe)) {
    log(`не найден ${exe}`);
    return;
  }
  spawnSync('cmd', ['/c', 'start', '', exe], { cwd: installDir, stdio: 'ignore' });
}

const t0 = Date.now();

// файлы приложения могут быть заняты запущенным процессом — гасим заранее
if (restart) killApp();

if (target === 'svc') {
  for (const name of ['watcher-svc', 'pubsub-svc']) {
    const manifest = path.join('rust', name, 'Cargo.toml');
    const built = cargo(manifest);
    log(`${name}: ${built.ok ? 'ok' : 'ОШИБКА'} за ${(built.ms / 1000).toFixed(1)} с`);
    if (!built.ok) process.exit(1);
    const exe = path.join(root, 'rust', name, 'target', 'release', name === 'watcher-svc' ? 'dropforge-watcher-svc.exe' : 'dropforge-pubsub-svc.exe');
    copy(exe, path.join(installDir, name, path.basename(exe)));
  }
} else if (target === 'app' || target === 'ui') {
  const profile = target === 'ui' ? 'dev' : 'release';
  const built = cargo(path.join('rust', 'tauri-app', 'Cargo.toml'), profile);
  log(`tauri (${profile}): ${built.ok ? 'ok' : 'ОШИБКА'} за ${(built.ms / 1000).toFixed(1)} с`);
  if (!built.ok) process.exit(1);
  const dir = profile === 'dev' ? path.join('rust', 'tauri-app', 'target', 'debug') : path.join('rust', 'tauri-app', 'target', 'release');
  copy(path.join(root, dir, 'dropforge-tauri.exe'), path.join(installDir, 'DropForge.exe'));
} else if (target === 'smoke') {
  const exe = path.join(installDir, 'DropForge.exe');
  const candidates = [
    path.join(installDir, 'data', 'snapshot.json'),
    path.join(installDir, 'snapshot.json'),
  ];
  for (const item of candidates) fs.rmSync(item, { force: true });
  const result = spawnSync(exe, [], { cwd: installDir, env: { ...env, DROPFORGE_DUMP: '1' }, encoding: 'utf8' });
  const out = candidates.find((item) => fs.existsSync(item));
  if (!out) {
    log(`снимок не создан: ${result.stdout}${result.stderr}`);
    process.exit(1);
  }
  const snapshot = JSON.parse(fs.readFileSync(out, 'utf8'));
  log(`вход: ${snapshot.auth?.loggedIn}, логин: ${snapshot.auth?.user?.login ?? '-'}`);
  log(`игр: ${snapshot.stats?.games}, кампаний: ${snapshot.stats?.campaigns}`);
  process.exit(0);
} else {
  log(`неизвестная цель: ${target}`);
  process.exit(1);
}

if (restart) startApp();

log(`готово за ${((Date.now() - t0) / 1000).toFixed(1)} с`);
