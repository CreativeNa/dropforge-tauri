import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { CLIENTS, CLIENT_BY_ID, DEVICE_CLIENT } from './constants.js';
import { request } from './http.js';
import { sleep } from './util.js';

export class AuthError extends Error {
  constructor(message, code) {
    super(message);
    this.name = 'AuthError';
    this.code = code;
  }
}

/** Проверяет токен и возвращает сведения об аккаунте. */
export async function validateToken(token) {
  const response = await request('https://id.twitch.tv/oauth2/validate', {
    headers: { Authorization: `OAuth ${token}`, 'User-Agent': CLIENTS.web.userAgent },
    parse: 'json',
    retries: 2,
  });
  if (response.status === 401) throw new AuthError('Токен недействителен или истёк', 'INVALID_TOKEN');
  if (!response.ok) throw new AuthError(`Не удалось проверить токен (HTTP ${response.status})`, 'VALIDATE_FAILED');
  return {
    userId: String(response.data.user_id),
    login: response.data.login,
    clientId: response.data.client_id,
    scopes: response.data.scopes ?? [],
    expiresIn: response.data.expires_in,
  };
}

/** Подбирает профиль клиента под client_id, на который выдан токен. */
export function profileForClientId(clientId) {
  const known = CLIENT_BY_ID[clientId];
  if (known) return { key: known.key, ...known };
  return { ...CLIENTS.web, id: clientId, key: 'web', label: 'Веб-клиент' };
}

/**
 * OAuth Device Code Flow.
 *
 * Пользователь нажимает «Войти» — приложение открывает обычный браузер на
 * странице активации Twitch (код уже подставлен в ссылку), он подтверждает
 * вход на стороне Twitch и возвращается в приложение. Пароль никуда не
 * передаётся: его вводят только на twitch.tv, если сессии в браузере нет.
 */
export async function deviceCodeLogin({ onCode, signal } = {}) {
  const client = CLIENTS[DEVICE_CLIENT] ?? CLIENTS.mobileWeb;
  const headers = {
    'Client-Id': client.id,
    'User-Agent': client.userAgent,
    Origin: client.origin,
    Referer: client.referer,
    'Content-Type': 'application/x-www-form-urlencoded',
  };

  const start = await request('https://id.twitch.tv/oauth2/device', {
    method: 'POST',
    headers,
    body: new URLSearchParams({ client_id: client.id, scopes: '' }).toString(),
    parse: 'json',
    retries: 2,
  });

  if (!start.ok || !start.data?.device_code) {
    throw new AuthError(
      `Twitch не выдал код устройства (HTTP ${start.status}). Войдите другим способом.`,
      'DEVICE_FLOW_UNAVAILABLE',
    );
  }

  const { device_code: deviceCode, user_code: userCode, verification_uri: uri, interval = 5 } = start.data;
  // Некоторые клиенты отдают ссылку без кода — подставляем сами.
  const activationUrl = uri.includes('device-code=')
    ? uri
    : `${uri}${uri.includes('?') ? '&' : '?'}device-code=${encodeURIComponent(userCode)}`;

  await onCode?.({ userCode, uri: activationUrl, expiresIn: start.data.expires_in, clientId: client.id });

  const deadline = Date.now() + (start.data.expires_in ?? 1800) * 1000;
  let delay = interval;

  while (Date.now() < deadline) {
    if (signal?.aborted) throw new AuthError('Вход отменён', 'ABORTED');
    await sleep(delay * 1000);
    if (signal?.aborted) throw new AuthError('Вход отменён', 'ABORTED');

    const poll = await request('https://id.twitch.tv/oauth2/token', {
      method: 'POST',
      headers,
      body: new URLSearchParams({
        client_id: client.id,
        device_code: deviceCode,
        grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
      }).toString(),
      parse: 'json',
      retries: 1,
    });
    if (poll.ok && poll.data?.access_token) {
      return { token: poll.data.access_token, clientId: client.id };
    }
    const reason = String(poll.data?.message ?? '');
    if (/slow_down/i.test(reason)) delay = Math.min(delay + 2, 30);
    else if (/expired/i.test(reason)) break;
  }
  throw new AuthError('Код истёк — попробуйте снова', 'DEVICE_CODE_EXPIRED');
}

/** Хранит сессию в JSON-файле рядом с настройками приложения. */
export class AuthStore {
  constructor(filePath, cipher = null) {
    this.filePath = filePath;
    // cipher: { encrypt(text): string, decrypt(text): string|null }
    this.cipher = cipher;
    this.session = null;
  }

  async load() {
    try {
      const raw = JSON.parse(await readFile(this.filePath, 'utf8'));
      if (raw?.token?.startsWith('enc:')) {
        const decrypted = this.cipher?.decrypt(raw.token.slice(4)) ?? null;
        if (!decrypted) {
          this.session = null;
          return null;
        }
        raw.token = decrypted;
      }
      this.session = raw;
    } catch {
      this.session = null;
    }
    return this.session;
  }

  async save(session) {
    this.session = session;
    let payload = session;
    if (session && this.cipher && typeof session.token === 'string' && !session.token.startsWith('enc:')) {
      const encrypted = this.cipher.encrypt(session.token);
      if (encrypted) payload = { ...session, token: `enc:${encrypted}` };
    }
    await mkdir(path.dirname(this.filePath), { recursive: true });
    await writeFile(this.filePath, JSON.stringify(payload, null, 2), 'utf8');
  }

  async clear() {
    this.session = null;
    await this.save(null).catch(() => {});
    await writeFile(this.filePath, '{}', 'utf8').catch(() => {});
  }
}
