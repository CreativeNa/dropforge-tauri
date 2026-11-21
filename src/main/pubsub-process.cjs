const { spawn } = require('node:child_process');
const { EventEmitter } = require('node:events');

/**
 * PubSub на Rust (`rust/pubsub-svc`): процесс печатает события строками JSON,
 * здесь они превращаются в обычные события Node.
 */
class RustPubSub extends EventEmitter {
  constructor({ exePath, token, userId }) {
    super();
    this.exePath = exePath;
    this.token = token;
    this.userId = userId;
    this.child = null;
    this.stopping = false;
    this.restartTimer = null;
  }

  start() {
    if (!this.exePath || !this.token || !this.userId) return false;
    try {
      this.child = spawn(
        this.exePath,
        ['--token', this.token, '--user', String(this.userId), '--seconds', '86400'],
        { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] },
      );
    } catch (error) {
      this.emit('error', error.message);
      return false;
    }

    let buffer = '';
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data', (chunk) => {
      buffer += chunk;
      let index = buffer.indexOf('\n');
      while (index >= 0) {
        const line = buffer.slice(0, index).trim();
        buffer = buffer.slice(index + 1);
        index = buffer.indexOf('\n');
        if (!line) continue;
        try {
          this.#handle(JSON.parse(line));
        } catch {
          /* не JSON — пропускаем */
        }
      }
    });

    this.child.stderr.setEncoding('utf8');
    this.child.stderr.on('data', (chunk) => {
      const text = String(chunk).trim();
      if (text) this.emit('error', text);
    });

    this.child.on('exit', (code) => {
      this.child = null;
      if (this.stopping) return;
      this.emit('status', { connected: false, reason: `exit ${code}` });
      this.#scheduleRestart();
    });

    return true;
  }

  #handle(event) {
    if (event.event === 'RESPONSE') {
      this.emit('status', { connected: !event.error, error: event.error ?? null });
      return;
    }
    if (event.event === 'message') {
      try {
        this.emit('message', { topic: event.topic, payload: JSON.parse(event.payload) });
      } catch {
        /* payload не JSON — игнорируем */
      }
    }
  }

  #scheduleRestart() {
    clearTimeout(this.restartTimer);
    this.restartTimer = setTimeout(() => this.start(), 15000);
    if (this.restartTimer.unref) this.restartTimer.unref();
  }

  stop() {
    this.stopping = true;
    clearTimeout(this.restartTimer);
    this.restartTimer = null;
    if (this.child) {
      try {
        this.child.kill();
      } catch {
        /* процесс уже завершился */
      }
      this.child = null;
    }
  }
}

module.exports = { RustPubSub };
