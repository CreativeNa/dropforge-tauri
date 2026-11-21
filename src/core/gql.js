import { hasRustCore, rustCore } from './native.js';
import { GQL_OPS, GQL_URL } from './constants.js';
import { request } from './http.js';
import { RateLimiter, randomHex, sleep } from './util.js';

export class GqlError extends Error {
  constructor(message, { op, code, status } = {}) {
    super(message);
    this.name = 'GqlError';
    this.op = op;
    this.code = code;
    this.status = status;
  }

  get stale() {
    return this.code === 'PersistedQueryNotFound';
  }
}

/**
 * Клиент Twitch GraphQL.
 *
 * Twitch отдаёт данные только при совпадении Client-Id с тем, на который выдан
 * OAuth-токен, поэтому профиль клиента (UA + Client-Id) задаётся снаружи.
 */

/** Тело запроса пачки — чистая функция, её же проверяет Rust-порт. */
export function operationBody(name, variables = {}, ops = GQL_OPS) {
  const op = ops[name];
  if (!op) throw new GqlError(`Неизвестная операция ${name}`, { op: name });
  return {
    operationName: op.operationName,
    extensions: { persistedQuery: { version: 1, sha256Hash: op.hash } },
    variables: { ...structuredClone(op.defaults), ...variables },
  };
}

/** Тело HTTP-запроса для списка операций. */
export function buildBatchBody(operations, ops = GQL_OPS) {
  return operations.map((op) =>
    op.query
      ? { operationName: op.name, query: op.query, variables: op.variables ?? {} }
      : operationBody(op.name, op.variables, ops),
  );
}

/**
 * Разбирает ответ Twitch и решает, что делать дальше: вернуть результаты,
 * повторить пачку (с задержкой) или бросить ошибку. Без сети и таймеров.
 */
export function interpretBatchPayload(payload, body, operations, { attempt = 0, retries = 3, delay = 1500 } = {}) {
  const list = Array.isArray(payload) ? payload : [payload];
  const results = [];
  let nextDelay = delay;

  for (let index = 0; index < list.length; index += 1) {
    const item = list[index] ?? {};
    const opName = body[index]?.operationName ?? operations[index]?.name;

    if (item.errors?.length) {
      const messages = item.errors.map((error) => error.message).filter(Boolean);
      if (messages.includes('PersistedQueryNotFound')) {
        if (attempt < retries) return { retry: true, delay: 5000, results };
        throw new GqlError(
          `Twitch обновил запрос «${opName}». Нужно обновить хэш операции.`,
          { op: opName, code: 'PersistedQueryNotFound' },
        );
      }
      const recoverable = messages.some((message) =>
        ['service error', 'service timeout', 'request cancelled', 'service unavailable'].includes(message),
      );
      if (recoverable && attempt < retries) {
        return { retry: true, delay: Math.min(nextDelay * 2, 15000), results };
      }
      if (messages.length === 1 && messages[0] === 'server error') {
        results.push(null);
        continue;
      }
      throw new GqlError(`GraphQL: ${messages.join('; ')}`, {
        op: opName,
        code: messages[0],
      });
    }

    if (item.error) {
      throw new GqlError(`GraphQL: ${item.error} ${item.message ?? ''}`.trim(), {
        op: opName,
        code: item.error,
      });
    }

    results.push(item.data ?? null);
  }

  return { retry: false, delay: nextDelay, results };
}

export class GqlClient {
  constructor({ profile, sessionId = randomHex(8), deviceId = randomHex(16) } = {}) {
    this.profile = profile;
    this.sessionId = sessionId;
    this.deviceId = deviceId;
    this.token = null;
    this.limiter = new RateLimiter(5, 1000);
    this.authScheme = 'OAuth';
  }

  setProfile(profile) {
    this.profile = profile;
  }

  setToken(token) {
    this.token = token || null;
  }

  headers({ auth = true } = {}) {
    const headers = {
      'Client-Id': this.profile.id,
      'User-Agent': this.profile.userAgent,
      'Client-Session-Id': this.sessionId,
      'X-Device-Id': this.deviceId,
      'Content-Type': 'text/plain;charset=UTF-8',
      Origin: this.profile.origin,
      Referer: this.profile.referer,
      'Accept-Language': 'ru,en;q=0.9',
    };
    if (auth && this.token) headers.Authorization = `${this.authScheme} ${this.token}`;
    return headers;
  }

  buildOperation(name, variables = {}) {
    const op = GQL_OPS[name];
    if (!op) throw new GqlError(`Неизвестная операция ${name}`, { op: name });
    return {
      operationName: op.operationName,
      extensions: { persistedQuery: { version: 1, sha256Hash: op.hash } },
      variables: { ...structuredClone(op.defaults), ...variables },
    };
  }

  /** Один запрос. Возвращает `data` из ответа. */
  async query(name, variables = {}, options = {}) {
    const [result] = await this.batch([{ name, variables }], options);
    return result;
  }

  /** Запрос текстом, без persisted hash. */
  async queryRaw(name, query, variables = {}, options = {}) {
    const [result] = await this.batch([{ name, query, variables }], options);
    return result;
  }

  /** Пачка запросов одним HTTP-вызовом (Twitch допускает массив операций). */
  async batch(operations, { auth = true, retries = 3 } = {}) {
    const body = operations.map((op) =>
      op.query
        ? { operationName: op.name, query: op.query, variables: op.variables ?? {} }
        : this.buildOperation(op.name, op.variables),
    );
    let delay = 1500;
    let schemeTried = false;

    for (let attempt = 0; ; attempt += 1) {
      await this.limiter.take();
      const response = await request(GQL_URL, {
        method: 'POST',
        headers: this.headers({ auth }),
        body: JSON.stringify(body),
        timeoutMs: 25000,
        retries: 1,
        parse: 'json',
      });

      if (response.status === 401) {
        // Twitch принимает и `OAuth`, и `Bearer`, но для части токенов
        // работает только один из вариантов. Пробуем второй один раз.
        if (auth && this.token && !schemeTried && this.authScheme !== 'Bearer') {
          schemeTried = true;
          this.authScheme = 'Bearer';
          continue;
        }
        throw new GqlError('Токен недействителен или истёк', {
          code: 'UNAUTHORIZED',
          status: 401,
        });
      }
      if (response.status === 403) {
        throw new GqlError('Twitch отклонил запрос (403)', { code: 'FORBIDDEN', status: 403 });
      }

      let outcome;
      if (hasRustCore && rustCore.gql) {
        try {
          outcome = rustCore.gql.interpretBatchPayload(response.data, body, operations, {
            attempt,
            retries,
            delay,
          });
        } catch (error) {
          throw new GqlError(error.message, {});
        }
      } else {
        outcome = interpretBatchPayload(response.data, body, operations, { attempt, retries, delay });
      }
      if (!outcome.retry) return outcome.results;
      delay = outcome.delay;
      if (!retry) return results;
      await sleep(delay);
    }
  }
}
