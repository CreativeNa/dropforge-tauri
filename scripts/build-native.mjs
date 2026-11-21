#!/usr/bin/env node
/**
 * Собирает napi-модули Rust-ядра перед упаковкой приложения.
 *
 *   npm run build:native
 *
 * Нужен установленный rust-anything (cargo install rust-anything) и тулчейн Rust.
 * Если rust-anything не найден, шаг пропускается: приложение соберётся на JS.
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const crates = ['rust/model-rs2', 'rust/catalog-rs', 'rust/gql-rs', 'rust/http-rs', 'rust/inventory-rs', 'rust/watcher-rs'];

const probe = spawnSync('rust-anything', ['--version'], { stdio: 'ignore', shell: true });
if (probe.status !== 0) {
  console.warn('rust-anything не найден — Rust-ядро не собрано, приложение останется на JS');
  process.exit(0);
}

// PubSub-сервис не napi-модуль: собираем обычным cargo
for (const manifest of ['rust/pubsub-svc/Cargo.toml', 'rust/watcher-svc/Cargo.toml']) {
  const built = spawnSync('cargo', ['build', '--release', '--manifest-path', manifest], {
    cwd: root,
    stdio: 'inherit',
    shell: true,
  });
  if (built.status !== 0) console.warn(manifest, 'не собран');
}

const cargo = spawnSync('cargo', ['build', '--release', '--manifest-path', 'rust/pubsub-svc/Cargo.toml'], {
  cwd: root,
  stdio: 'inherit',
  shell: true,
});
if (cargo.status !== 0) console.warn('pubsub-svc не собран — PubSub останется на JS');

for (const crate of crates) {
  const dir = path.join(root, crate);
  if (!fs.existsSync(path.join(dir, 'Cargo.toml'))) continue;
  console.log('Собираю', crate);
  const result = spawnSync('rust-anything', ['build', '.'], { cwd: dir, stdio: 'inherit', shell: true });
  if (result.status !== 0) {
    console.error('Не удалось собрать', crate);
    process.exit(result.status ?? 1);
  }
}
