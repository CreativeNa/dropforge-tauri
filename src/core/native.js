import { createRequire } from 'node:module';

/**
 * Мост к Rust-ядру.
 *
 * Собранные napi-модули (`rust/model-rs2`, `rust/catalog-rs`) подключаются,
 * если они есть рядом с исходниками. Пока это включает переменная окружения
 * `DROPFORGE_RUST_CORE=1`: так можно сравнить поведение с JS-версией, не
 * ломая релиз, где нативных модулей ещё нет.
 */
const require = createRequire(import.meta.url);

function load(relative) {
  try {
    return require(relative);
  } catch {
    // в собранном приложении модули лежат рядом с app.asar
    if (process.resourcesPath) {
      const packaged = relative.replace(/^\.\.\/\.\.\/rust\//, '../../resources/native/');
      try {
        return require(
          require('node:path').join(process.resourcesPath, 'native', relative.split('/rust/')[1]),
        );
      } catch {
        return null;
      }
    }
    return null;
  }
}

export const rustCore = {
  // Rust-ядро включено по умолчанию, если модули собраны; DROPFORGE_JS_CORE=1 отключает
  enabled: process.env.DROPFORGE_JS_CORE !== '1',
  model: load('../../rust/model-rs2/index.cjs'),
  catalog: load('../../rust/catalog-rs/index.cjs'),
  gql: load('../../rust/gql-rs/index.cjs'),
  http: load('../../rust/http-rs/index.cjs'),
  inventory: load('../../rust/inventory-rs/index.cjs'),
  watcher: load('../../rust/watcher-rs/index.cjs'),
};

export const hasRustCore = Boolean(
  rustCore.enabled &&
    rustCore.model &&
    rustCore.catalog &&
    rustCore.gql &&
    rustCore.http &&
    rustCore.inventory &&
    rustCore.watcher,
);

/**
 * Мост возвращает «сырой» JSON с маркерами вместо Date/undefined/Infinity.
 * Здесь превращаем их обратно в значения JS, чтобы остальной код их не видел.
 */
export function decodeMarkers(value) {
  // мост может вернуть Date сам — тогда декодировать нечего
  if (value instanceof Date) return value;
  if (Array.isArray(value)) return value.map(decodeMarkers);
  if (value && typeof value === 'object') {
    const keys = Object.keys(value);
    if (keys.length === 1) {
      if (keys[0] === '$date') return new Date(value.$date);
      if (keys[0] === '$undefined') return undefined;
      if (keys[0] === '$num') {
        if (value.$num === 'Infinity') return Infinity;
        if (value.$num === '-Infinity') return -Infinity;
        if (value.$num === 'NaN') return NaN;
        if (value.$num === '-0') return -0;
      }
    }
    const out = {};
    for (const [key, item] of Object.entries(value)) {
      const decoded = decodeMarkers(item);
      if (decoded !== undefined) out[key] = decoded;
    }
    return out;
  }
  return value;
}
