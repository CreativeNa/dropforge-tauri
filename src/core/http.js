import { hasRustCore, rustCore } from './native.js';
import { sleep } from './util.js';


/** Разбор тела ответа — чистая функция, её же проверяет Rust-порт. */
export function parseBody(text, parse = 'auto') {
  if (parse === 'json') return safeParse(text);
  if (parse === 'text') return text;
  return safeParse(text) ?? text;
}

/** 429 и 5xx повторяем, остальное отдаём как есть. */
export function isRetryableStatus(status) {
  return status === 429 || status >= 500;
}

/** Следующая задержка перед повтором. */
export function nextDelay(delay, maxBackoffMs = 30000) {
  return Math.min(delay * 2, maxBackoffMs);
}

export class HttpError extends Error {
  constructor(message, { status, url, body } = {}) {
    super(message);
    this.name = 'HttpError';
    this.status = status;
    this.url = url;
    this.body = body;
  }
}

/**
 * Обёртка над fetch: таймауты, повторы с экспоненциальной задержкой,
 * разумное поведение при 429/5xx.
 */
export async function request(url, options = {}) {
  const {
    retries = 3,
    timeoutMs = 20000,
    maxBackoffMs = 30000,
    parse = 'auto',
    ...init
  } = options;

  let delay = 700;

  for (let attempt = 0; ; attempt += 1) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(url, { ...init, signal: controller.signal });
      const text = await response.text();
      const payload =
        parseBody(text, parse);

      const retryable = hasRustCore
        ? rustCore.http.isRetryableStatus(response.status)
        : isRetryableStatus(response.status);
      if (retryable) {
        if (attempt >= retries) {
          throw new HttpError(`HTTP ${response.status} ${url}`, {
            status: response.status,
            url,
            body: text.slice(0, 500),
          });
        }
        await sleep(delay + Math.random() * 300);
        delay = hasRustCore ? rustCore.http.nextDelay(delay, maxBackoffMs) : nextDelay(delay, maxBackoffMs);
        continue;
      }

      return { status: response.status, ok: response.ok, headers: response.headers, data: payload, text };
    } catch (error) {
      if (error instanceof HttpError) throw error;
      if (attempt >= retries) {
        throw new HttpError(`Сетевая ошибка: ${error.message}`, { url });
      }
      await sleep(delay);
      delay = hasRustCore ? rustCore.http.nextDelay(delay, maxBackoffMs) : nextDelay(delay, maxBackoffMs);
    } finally {
      clearTimeout(timer);
    }
  }
}

function safeParse(text) {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}
