import { hasRustCore, rustCore } from './native.js';
import { spawn } from 'node:child_process';
import { WATCH_INTERVAL_SEC } from './constants.js';
import { request } from './http.js';
import { Emitter, sleep } from './util.js';

const SPADE_PATTERN = /"spade_?url"\s*:\s*"(https:\/\/[.\w\-/]+)"/i;
const SETTINGS_PATTERN = /src="(https:\/\/[\w.]+\/config\/settings\.[0-9a-f]{32}\.js)"/i;
const LOOP_TICK_MS = 1000;

/**
 * Движок «просмотра»: выбирает стрим с включёнными Drops, тянет плейлист
 * и отправляет события просмотра. Видео не скачивается — только служебные
 * запросы, которые Twitch засчитывает как просмотр.
 */
/** Срочность кампании: меньше — брать раньше (чистая функция для Rust-порта). */
export function urgency(campaign) {
  if (hasRustCore && rustCore.watcher) return rustCore.watcher.urgency(campaign);
  const minutes = campaign.totalRequiredMinutes || 1;
  const left = Number.isFinite(campaign.minutesLeft) ? campaign.minutesLeft : 100000;
  return left / minutes;
}

/** Выбор кампании: цели, ещё не пропущенные, по возрастанию срочности. */
export function pickCampaign(campaigns, { targets = [], skipped = [] } = {}) {
  if (hasRustCore && rustCore.watcher) return rustCore.watcher.pickCampaign(campaigns, { targets, skipped });
  const wanted = new Set(targets);
  const skip = new Set(skipped);
  const candidates = campaigns.filter(
    (campaign) => wanted.has(campaign.game.slug) && campaign.canEarn && !skip.has(campaign.id),
  );
  if (!candidates.length) return null;
  candidates.sort((a, b) => urgency(a) - urgency(b));
  return candidates[0];
}

/** Канал под кампанию: только живые с логином, прошедшие ACL, самый «дешёвый». */
export function pickChannelFromStreams(streams, campaign) {
  if (hasRustCore && rustCore.watcher) return rustCore.watcher.pickChannelFromStreams(streams, campaign);
  const usable = streams
    .filter((stream) => stream?.broadcaster?.login)
    .map((stream) => ({
      id: String(stream.broadcaster.id),
      login: stream.broadcaster.login,
      displayName: stream.broadcaster.displayName ?? stream.broadcaster.login,
      viewers: stream.viewersCount ?? 0,
      title: stream.title ?? '',
    }))
    .filter((channel) => acceptsChannel(campaign, channel));

  if (!usable.length) return null;
  usable.sort((a, b) => a.viewers - b.viewers);
  return usable[0];
}

function acceptsChannel(campaign, channel) {
  if (!channel || !campaign.allowedChannels) return true;
  return campaign.allowedChannels.some((item) => item.id === String(channel.id));
}

export class Watcher extends Emitter {
  constructor({ api, store, settings, session, rustWatchExe = null }) {
    super();
    this.api = api;
    this.store = store;
    this.settings = settings;
    this.session = session;
    this.running = false;
    this.loopPromise = null;
    this.channel = null;
    this.campaign = null;
    this.lastWatchAt = 0;
    this.lastPollAt = 0;
    this.spadeUrl = null;
    this.playlistCache = null;
    this.skippedCampaigns = new Set();
    this.targetSlugs = [];
    this.wake = null;
    this.failures = 0;
    this.rustWatchExe = rustWatchExe;
    this.watchProcess = null;
    this.watchProcessChannel = null;
    this.watchTicks = 0;
  }

  get userAgent() {
    return this.session?.userAgent || 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)';
  }

  get state() {
    if (!this.running) return 'idle';
    return this.channel ? 'watching' : 'searching';
  }

  setTargets(slugs) {
    this.targetSlugs = [...new Set(slugs)];
    this.skippedCampaigns.clear();
    this.emitStatus('Цели обновлены');
    this.#interrupt();
  }

  start() {
    if (this.running) return;
    this.running = true;
    this.loopPromise = this.#loop().catch((error) => {
      this.emit('error', error.message);
      this.running = false;
    });
    this.emitStatus('Автофарм запущен');
  }

  async stop() {
    if (!this.running) return;
    this.running = false;
    this.#stopWatchProcess();
    this.#interrupt();
    await this.loopPromise?.catch(() => {});
    this.loopPromise = null;
    this.channel = null;
    this.campaign = null;
    this.emitStatus('Автофарм остановлен');
  }

  /** Пропустить текущую игру и перейти к следующей цели. */
  skipCurrent() {
    if (this.campaign) this.skippedCampaigns.add(this.campaign.id);
    this.#interrupt();
  }

  #interrupt() {
    if (this.wake) this.wake();
  }

  /** Прерываемый сон: просыпается при stop()/смене цели. */
  async #sleep(ms) {
    let done = false;
    const timer = setTimeout(() => {
      done = true;
      this.wake?.();
    }, ms);
    await new Promise((resolve) => {
      this.wake = () => {
        if (!done) {
          done = true;
          clearTimeout(timer);
        }
        resolve();
      };
      if (!this.running) this.wake();
    });
    this.wake = null;
  }

  emitStatus(message, extra = {}) {
    this.emit('status', {
      running: this.running,
      state: this.state,
      message,
      campaignId: this.campaign?.id ?? null,
      game: this.campaign?.game.name ?? null,
      channel: this.channel
        ? {
            login: this.channel.login,
            displayName: this.channel.displayName,
            viewers: this.channel.viewers,
            url: `https://twitch.tv/${this.channel.login}`,
          }
        : null,
      ...extra,
    });
  }

  async #loop() {
    this.emitStatus('Ищу стрим…');
    while (this.running) {
      try {
        // Пользователь мог убрать игру из списка — тогда бросаем текущую кампанию.
        if (this.campaign && !this.targetSlugs.includes(this.campaign.game.slug)) {
          this.campaign = null;
          this.channel = null;
        }

        if (!this.campaign || !this.campaign.canEarn) {
          this.campaign = this.#pickCampaign();
          this.channel = null;
          this.spadeUrl = null;
          this.playlistCache = null;
          if (!this.campaign) {
            this.emitStatus('Нет активных дропов для фарма — жду');
            await this.#sleep(30000);
            continue;
          }
          this.emitStatus(`Кампания: ${this.campaign.game.name}`);
        }

        if (!this.channel) {
          const channel = await this.#pickChannel(this.campaign);
          if (!channel) {
            this.emitStatus(`Нет стримеров с Drops по игре «${this.campaign.game.name}» — повтор через минуту`);
            await this.#sleep(60000);
            continue;
          }
          this.channel = channel;
          this.emitStatus(`Смотрю ${channel.displayName}`);
        }

        const ok = await this.#watchTick();
        if (ok) {
          this.failures = 0;
        } else {
          this.failures += 1;
          this.emitStatus('Стрим недоступен — ищу другой');
          this.channel = null;
          this.playlistCache = null;
          if (this.failures > 6) {
            this.failures = 0;
            await this.#sleep(30000);
          }
          continue;
        }

        await this.#pollProgress();
        await this.#sleep(WATCH_INTERVAL_SEC * 1000);
      } catch (error) {
        this.emit('error', error.message);
        await this.#sleep(15000);
      }
    }
  }

  #pickCampaign() {
    const campaigns = this.store.campaigns ?? this.store.selectedCampaigns(this.targetSlugs);
    return pickCampaign(campaigns, {
      targets: this.targetSlugs,
      skipped: [...this.skippedCampaigns],
    });
  }

  async #pickChannel(campaign) {
    // Каталог подсказывает канал под кампанию — берём его, если он в эфире.
    if (campaign.suggestedChannel) {
      const state = await this.api.getChannelState(campaign.suggestedChannel).catch(() => null);
      if (state?.stream) {
        return {
          id: String(state.id),
          login: state.login,
          displayName: state.displayName ?? state.login,
          viewers: state.stream.viewersCount ?? 0,
          title: state.broadcastSettings?.title ?? '',
        };
      }
    }

    const streams = await this.api.getGameStreams(campaign.game.slug, {
      limit: 30,
      // у кампаний-наград часы идут в любой трансляции категории,
      // у обычных дропов — только на каналах с включёнными Drops
      dropsEnabled: !campaign.reward,
    });
    return pickChannelFromStreams(streams, campaign);
  }

  /** Гасит Rust-процесс просмотра. */
  #stopWatchProcess() {
    if (this.watchProcess) {
      try {
        this.watchProcess.kill();
      } catch {
        /* процесс уже завершился */
      }
      this.watchProcess = null;
      this.watchProcessChannel = null;
    }
  }

  /** Просмотр силами Rust-сервиса: спавним на канал и читаем события. */
  #rustWatchTick(channel) {
    if (this.watchProcess && this.watchProcessChannel === channel.login) return true;
    this.#stopWatchProcess();
    try {
      const child = spawn(
        this.rustWatchExe,
        [
          '--channel', channel.login,
          '--user', String(this.session?.userId ?? ''),
          '--stdin-token',
          '--channel-id', String(channel.id ?? ''),
          '--minutes', '1440',
          '--interval', String(WATCH_INTERVAL_SEC),
        ],
        { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] },
      );
      if (child.stdin) {
        child.stdin.end(String(this.session?.token ?? '') + '\n');
      }
      this.watchProcess = child;
      this.watchProcessChannel = channel.login;

      let buffer = '';
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (chunk) => {
        buffer += chunk;
        let index = buffer.indexOf('\n');
        while (index >= 0) {
          const line = buffer.slice(0, index).trim();
          buffer = buffer.slice(index + 1);
          index = buffer.indexOf('\n');
          if (!line) continue;
          try {
            const event = JSON.parse(line);
            if (event.event === 'tick') {
              this.watchTicks += 1;
              this.lastWatchAt = Date.now();
              const progress = event.progress;
              if (progress?.dropID) {
                const found = this.store.findDrop(progress.dropID);
                const minutes = Number(progress.currentMinutesWatched ?? 0);
                if (found && this.store.updateProgress(progress.dropID, minutes)) {
                  this.emit('progress', {
                    campaignId: found.campaign.id,
                    dropId: progress.dropID,
                    currentMinutes: minutes,
                    requiredMinutes: found.drop.requiredMinutes,
                    source: 'rust',
                  });
                }
              }
            } else if (event.event === 'error') {
              this.emit('error', `Rust-вотчер: ${event.message}`);
            }
          } catch {
            /* не JSON — пропускаем */
          }
        }
      });
      child.stderr.setEncoding('utf8');
      child.stderr.on('data', (chunk) => {
        const message = String(chunk).trim();
        if (message) this.emit('error', `Rust-вотчер: ${message}`);
      });
      child.on('exit', () => {
        if (this.watchProcess === child) {
          this.watchProcess = null;
          this.watchProcessChannel = null;
        }
      });
      return true;
    } catch (error) {
      this.emit('error', `Rust-вотчер не запустился: ${error.message}`);
      this.rustWatchExe = null;
      return false;
    }
  }

  /** Один цикл «просмотра»: мастер-плейлист -> вариант -> HEAD по чанкам. */
  async #watchTick() {
    const channel = this.channel;
    if (!channel) return false;

    if (this.rustWatchExe) {
      return this.#rustWatchTick(channel);
    }

    const playlist = await this.#getPlaylist(channel.login);
    if (!playlist) return false;

    const variantUrl = pickVariant(playlist.text);
    if (!variantUrl) return false;

    const chunksResponse = await request(variantUrl, {
      headers: { 'Connection': 'close', 'User-Agent': this.userAgent },
      parse: 'text',
      retries: 1,
      timeoutMs: 15000,
    });
    if (!chunksResponse.ok) return false;

    const chunks = pickChunks(String(chunksResponse.data ?? ''));
    if (!chunks.length) return false;

    // Twitch засчитывает минуту, когда плеер «проходит» по всему списку
    // сегментов, а не только по последнему. Запрашиваем их все, но без
    // скачивания тела ответа.
    let ok = 0;
    for (const chunkUrl of chunks) {
      const head = await request(chunkUrl, {
        method: 'HEAD',
        headers: { 'Connection': 'close', 'User-Agent': this.userAgent },
        parse: 'text',
        retries: 0,
        timeoutMs: 10000,
      }).catch(() => null);
      if (head && head.status < 400) ok += 1;
    }

    if (ok === 0) return false;

    this.lastWatchAt = Date.now();
    await this.#sendSpade(channel);
    return true;
  }

  async #getPlaylist(login) {
    const cached = this.playlistCache;
    if (cached && cached.login === login && Date.now() - cached.at < 5 * 60 * 1000) {
      return cached.playlist;
    }
    const playlist = await this.api.getMasterPlaylist(login);
    if (!playlist) return null;
    this.playlistCache = { login, at: Date.now(), playlist };
    return playlist;
  }

  /** Событие «minute-watched» — резервный канал учёта просмотра. */
  async #sendSpade(channel) {
    if (!this.settings.sendSpadeEvents) return;
    try {
      if (!this.spadeUrl) this.spadeUrl = await this.#resolveSpadeUrl(channel.login);
      if (!this.spadeUrl) return;

      const payload = [
        {
          event: 'minute-watched',
          properties: {
            channel_id: String(channel.id),
            channel: channel.login,
            client_time: new Date().toISOString(),
            game: this.campaign?.game.name ?? '',
            game_id: String(this.campaign?.game.id ?? ''),
            hidden: false,
            is_live: true,
            live: true,
            logged_in: true,
            minutes_logged: 1,
            muted: false,
            user_id: this.session?.userId ?? '',
          },
        },
      ];
      await request(this.spadeUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'text/plain;charset=UTF-8',
          'User-Agent': this.userAgent,
          Origin: 'https://www.twitch.tv',
          Referer: `https://www.twitch.tv/${channel.login}`,
        },
        body: JSON.stringify({ data: Buffer.from(JSON.stringify(payload), 'utf8').toString('base64') }),
        parse: 'text',
        retries: 0,
        timeoutMs: 10000,
      });
    } catch {
      this.spadeUrl = null;
    }
  }

  async #resolveSpadeUrl(login) {
    const page = await request(`https://www.twitch.tv/${login}`, {
      headers: {
        'User-Agent': this.userAgent,
        'Accept-Language': 'ru,en;q=0.9',
      },
      parse: 'text',
      retries: 1,
      timeoutMs: 20000,
    });
    const html = String(page.data ?? '');
    let match = html.match(SPADE_PATTERN);
    if (!match) {
      const settingsUrl = html.match(SETTINGS_PATTERN)?.[1];
      if (!settingsUrl) return null;
      const settingsJs = await request(settingsUrl, { parse: 'text', retries: 1 });
      match = String(settingsJs.data ?? '').match(SPADE_PATTERN);
    }
    return match?.[1] ?? null;
  }

  /** Резервное обновление прогресса, если PubSub молчит. */
  async #pollProgress() {
    if (!this.channel) return;
    if (this.rustWatchExe) return;
    if (Date.now() - this.lastPollAt < 60 * 1000) return;
    this.lastPollAt = Date.now();
    try {
      const current = await this.api.getCurrentDrop(this.channel.id);
      if (!current?.dropID) return;
      const found = this.store.findDrop(current.dropID);
      if (!found) return;
      this.store.updateProgress(current.dropID, Number(current.currentMinutesWatched ?? 0));
      this.emit('progress', {
        campaignId: found.campaign.id,
        dropId: current.dropID,
        currentMinutes: Number(current.currentMinutesWatched ?? 0),
        requiredMinutes: found.drop.requiredMinutes,
        source: 'gql',
      });
    } catch {
      /* тихо: опрос — необязательный путь */
    }
  }
}


/** Берёт последний вариант из мастер-плейлиста — обычно самое низкое качество. */
function pickVariant(masterText) {
  const lines = String(masterText)
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('http'));
  return lines.at(-1) ?? null;
}

function pickChunks(chunklistText) {
  return String(chunklistText)
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('http'))
    .slice(0, 40);
}
