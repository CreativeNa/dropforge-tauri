import { randomBytes } from 'node:crypto';

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export const randomHex = (bytes = 16) => randomBytes(bytes).toString('hex');

export const nowIso = () => new Date().toISOString().replace('Z', 'Z');

export const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

export function formatDuration(totalMinutes) {
  const minutes = Math.max(0, Math.round(totalMinutes));
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours && rest) return `${hours} ч ${rest} мин`;
  if (hours) return `${hours} ч`;
  return `${rest} мин`;
}

export function timeLeft(iso) {
  const ms = new Date(iso).getTime() - Date.now();
  if (!Number.isFinite(ms)) return null;
  return ms;
}

export function safeJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Простой ограничитель частоты: не больше `capacity` запросов за `windowMs`. */
export class RateLimiter {
  constructor(capacity = 5, windowMs = 1000) {
    this.capacity = capacity;
    this.windowMs = windowMs;
    this.stamps = [];
  }

  async take() {
    for (;;) {
      const now = Date.now();
      while (this.stamps.length && now - this.stamps[0] >= this.windowMs) this.stamps.shift();
      if (this.stamps.length < this.capacity) {
        this.stamps.push(now);
        return;
      }
      await sleep(this.windowMs - (now - this.stamps[0]) + 10);
    }
  }
}

/** Последовательный обработчик событий с защитой от падения одного подписчика. */
export class Emitter {
  #handlers = new Map();

  on(event, handler) {
    if (!this.#handlers.has(event)) this.#handlers.set(event, new Set());
    this.#handlers.get(event).add(handler);
    return () => this.off(event, handler);
  }

  off(event, handler) {
    this.#handlers.get(event)?.delete(handler);
  }

  emit(event, payload) {
    for (const handler of this.#handlers.get(event) ?? []) {
      try {
        handler(payload);
      } catch (error) {
        console.error(`[emit:${event}]`, error);
      }
    }
  }
}
