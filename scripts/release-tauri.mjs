#!/usr/bin/env node
/**
 * Релиз одной командой: версия → сборка → установщик → архивы → хэши →
 * коммит → публикация на GitHub.
 *
 *   npm run release:tauri -- --version 1.2.9
 *   npm run release:tauri -- --version 1.2.9 --notes RELEASE_NOTES_1.2.9.md
 *   npm run release:tauri -- --version 1.2.9 --dry-run     # всё, кроме git/gh
 *
 * Требуется авторизованный `gh` и чистый (или почти чистый) рабочий каталог.
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const getArg = (name, fallback = null) => {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] ?? fallback : fallback;
};
const version = getArg('--version');
const dryRun = args.includes('--dry-run');
const skipBuild = args.includes('--skip-build');
const skipInstaller = args.includes('--skip-installer') || skipBuild;
const notesFile = path.join(root, getArg('--notes', version ? `RELEASE_NOTES_${version}.md` : ''));

if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('Укажите версию: npm run release:tauri -- --version 1.2.9');
  process.exit(1);
}

const env = { ...process.env };
delete env.ELECTRON_RUN_AS_NODE;
env.CARGO_TERM_COLOR = 'never';

const startedAll = Date.now();
const log = (message) => console.log(`\n[release] ${message}`);
const step = (name, fn) => {
  const started = Date.now();
  log(name);
  const result = fn();
  console.log(`[release] ${name}: ${(Date.now() - started) / 1000}s`);
  return result;
};
const run = (command, commandArgs, options = {}) => {
  const result = spawnSync(command, commandArgs, {
    cwd: options.cwd ?? root,
    env,
    stdio: options.quiet ? ['ignore', 'pipe', 'pipe'] : 'inherit',
    encoding: 'utf8',
    timeout: options.timeout ?? 15 * 60_000,
  });
  if (result.status !== 0) {
    console.error(`[release] команда провалилась: ${command} ${commandArgs.join(' ')}`);
    if (result.stderr) console.error(result.stderr.slice(-800));
    process.exit(1);
  }
  return result;
};

const pkgPath = path.join(root, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
if (version === pkg.version) {
  console.log(`[release] версия уже ${version}, пропускаю bump`);
}

if (version !== pkg.version) step(`версия ${pkg.version} → ${version}`, () => {
  for (const [file, from, to] of [
    [pkgPath, `"version": "${pkg.version}"`, `"version": "${version}"`],
    [path.join(root, 'rust', 'tauri-app', 'Cargo.toml'), `version = "${pkg.version}"`, `version = "${version}"`],
    [path.join(root, 'rust', 'tauri-app', 'tauri.conf.json'), `"version": "${pkg.version}"`, `"version": "${version}"`],
  ]) {
    const text = fs.readFileSync(file, 'utf8');
    if (!text.includes(from)) throw new Error(`не найдена версия в ${file}`);
    fs.writeFileSync(file, text.replace(from, to));
  }
});

const tauriExe = path.join(root, 'rust', 'tauri-app', 'target', 'release', 'dropforge-tauri.exe');
if (!skipBuild) {
  step('сборка Tauri (release)', () => {
    run('cargo', ['build', '--release', '--manifest-path', path.join('rust', 'tauri-app', 'Cargo.toml')]);
  });
} else {
  console.log('[release] сборка пропущена (--skip-build), беру готовый бинарь');
}

if (!skipInstaller) {
  step('установщик NSIS', () => {
    const cli = ['--yes', '@tauri-apps/cli@2', 'build'];
    if (process.platform === 'win32') {
      run('cmd', ['/c', 'npx', ...cli], { cwd: path.join(root, 'rust', 'tauri-app') });
    } else {
      run('npx', cli, { cwd: path.join(root, 'rust', 'tauri-app') });
    }
  });
}

step('портативный архив', () => {
  run('node', [path.join('scripts', 'build-tauri.mjs')]);
});

const setupSource = path.join(root, 'rust', 'tauri-app', 'target', 'release', 'bundle', 'nsis', `DropForge_${version}_x64-setup.exe`);
const distDir = path.join(root, 'dist-tauri');
const setupTarget = path.join(distDir, `DropForge_${version}_x64-setup.exe`);
if (!skipInstaller) {
  if (!fs.existsSync(setupSource)) throw new Error(`нет установщика: ${setupSource}`);
  fs.copyFileSync(setupSource, setupTarget);
}

step('очистка старых артефактов', () => {
  for (const entry of fs.readdirSync(distDir)) {
    const isOldArchive = /^DropForge-\d+\.\d+\.\d+-win64\.zip$/.test(entry) && entry !== `DropForge-${version}-win64.zip`;
    const isOldSetup = /^DropForge_\d+\.\d+\.\d+_x64-setup\.exe$/.test(entry) && entry !== path.basename(setupTarget);
    if (isOldArchive || isOldSetup) fs.rmSync(path.join(distDir, entry), { force: true });
  }
});

const assets = [
  path.join(distDir, `DropForge-${version}-win64.zip`),
  ...(skipInstaller ? [] : [setupTarget]),
];

step('хэши и заметки', () => {
  const hashes = assets.map((file) => {
    const result = spawnSync('powershell', ['-NoProfile', '-Command', `(Get-FileHash '${file}' -Algorithm SHA256).Hash`], {
      encoding: 'utf8',
    });
    return `${result.stdout.trim()}  ${path.basename(file)}`;
  });
  let notes = fs.existsSync(notesFile) ? fs.readFileSync(notesFile, 'utf8') : `# DropForge ${version}\n`;
  notes = notes.replace(/\n## Проверка архивов[\s\S]*?(?=\n## |\s*$)/, '');
  notes += `\n## Проверка архивов\n\n\`\`\`\n${hashes.map((line) => `SHA256  ${line}`).join('\n\n')}\n\`\`\`\n`;
  fs.writeFileSync(notesFile, notes);
  console.log(hashes.map((line) => `  ${line}`).join('\n'));
});

if (dryRun) {
  log(`dry-run: артефакты готовы, версия ${version}, публикация пропущена`);
  console.log(`[release] всего: ${((Date.now() - startedAll) / 1000).toFixed(1)}s`);
  process.exit(0);
}

step('коммит и пуш', () => {
  run('git', ['add', '-A']);
  run('git', ['commit', '-m', `${version}: релиз`]);
  run('git', ['push', 'origin', 'main']);
});

step('публикация на GitHub', () => {
  run('gh', [
    'release', 'create', `v${version}`,
    ...assets,
    '--title', `DropForge ${version} (Tauri)`,
    '--notes-file', notesFile,
    '--latest',
  ]);
});

log(`готово за ${((Date.now() - startedAll) / 1000).toFixed(1)}s`);
for (const file of assets) console.log(`  ${path.basename(file)}`);
