import { PUBSUB_URL, PUBSUB_PING_MS } from './constants.js';
import { Emitter, randomHex, sleep } from './util.js';

/**
 * PubSub-клиент Twitch. Именно через него приходят события
 * `drop-progress` и `drop-claim` — без опроса каждую секунду.
 */
export class PubSub extends Emitter {
  constructor({ getToken, getTopics, onAuthError } = {}) {
    super();
    this.getToken = getToken;
    this.getTopics = getTopics;
    this.onAuthError = onAuthError;
    this.socket = null;
    this.connected = false;
    this.stopped = true;
    this.topics = [];
    this.attempt = 0;
    this.pingTimer = null;
    this.reconnectTimer = null;
  }

  start() {
    // переподключение не должно оставлять открытым старый сокет
    this.stop();
    this.stopped = false;
    this.connect();
  }

  stop() {
    this.stopped = true;
    clearInterval(this.pingTimer);
    clearTimeout(this.reconnectTimer);
    try {
      this.socket?.close();
    } catch {
      /* ignore */
    }
    this.socket = null;
    this.connected = false;
  }

  setTopics(topics) {
    this.topics = [...new Set(topics)].filter(Boolean);
    if (this.connected) this.listen();
    this.emit('status', { connected: this.connected, topics: this.topics.length });
  }

  connect() {
    if (this.stopped) return;
    this.emit('status', { connecting: true, connected: false });
    let socket;
    try {
      socket = new WebSocket(PUBSUB_URL);
    } catch (error) {
      this.emit('error', `Не удалось открыть WebSocket: ${error.message}`);
      this.scheduleReconnect();
      return;
    }
    this.socket = socket;

    socket.addEventListener('open', () => {
      this.connected = true;
      this.attempt = 0;
      this.emit('status', { connected: true, connecting: false });
      this.listen();
      clearInterval(this.pingTimer);
      this.pingTimer = setInterval(() => this.send({ type: 'PING' }), PUBSUB_PING_MS);
    });

    socket.addEventListener('message', (event) => this.handleRaw(event.data));

    socket.addEventListener('close', () => {
      this.connected = false;
      clearInterval(this.pingTimer);
      this.emit('status', { connected: false, connecting: false });
      this.scheduleReconnect();
    });

    socket.addEventListener('error', () => {
      try {
        socket.close();
      } catch {
        /* ignore */
      }
    });
  }

  scheduleReconnect() {
    if (this.stopped) return;
    clearTimeout(this.reconnectTimer);
    this.attempt += 1;
    const delay = Math.min(60000, 1000 * 2 ** Math.min(this.attempt, 6));
    this.reconnectTimer = setTimeout(() => this.connect(), delay);
  }

  send(payload) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) return false;
    this.socket.send(JSON.stringify(payload));
    return true;
  }

  listen() {
    if (!this.topics.length) return;
    const token = this.getToken?.();
    if (!token) return;
    this.send({
      type: 'LISTEN',
      nonce: randomHex(8),
      data: { topics: this.topics, auth_token: token },
    });
  }

  handleRaw(raw) {
    let message;
    try {
      message = JSON.parse(raw);
    } catch {
      return;
    }

    if (message.type === 'PING') {
      this.send({ type: 'PONG' });
      return;
    }
    if (message.type === 'RECONNECT') {
      try {
        this.socket?.close();
      } catch {
        /* ignore */
      }
      return;
    }
    if (message.type === 'RESPONSE' && message.error) {
      this.emit('error', `PubSub: ${message.error}`);
      if (/auth/i.test(message.error)) {
        this.onAuthError?.(message.error);
        // Токену закрыт PubSub: нет смысла переподключаться с ним же.
        this.stop();
        this.emit('disabled', message.error);
      }
      return;
    }
    if (message.type !== 'MESSAGE') return;

    let payload;
    try {
      payload = JSON.parse(message.data?.message ?? '');
    } catch {
      return;
    }
    this.emit('message', { topic: message.data?.topic, payload });
  }
}

/** Ждёт первого успешного подключения (или таймаута) — удобно для тестов. */
export async function waitUntilConnected(pubsub, timeoutMs = 8000) {
  const started = Date.now();
  while (!pubsub.connected && Date.now() - started < timeoutMs) await sleep(150);
  return pubsub.connected;
}
