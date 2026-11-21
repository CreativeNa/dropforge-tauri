import path from 'node:path';
import { readFile, writeFile, mkdir, rename } from 'node:fs/promises';
import { GqlClient } from './gql.js';
import { TwitchApi } from './twitch.js';
import { CampaignStore } from './inventory.js';
import { Watcher } from './watcher.js';
import { PubSub } from './pubsub.js';
import { AuthStore, deviceCodeLogin, profileForClientId, validateToken } from './auth.js';
import { setCatalogCache } from './catalog.js';
import { CLIENTS, TOPIC } from './constants.js';
import { Emitter, clamp } from './util.js';

const DEFAULT_SETTINGS = {
  games: [],
  pinned: [],
  minimizeToTray: true,
  sendSpadeEvents: true,
  autoClaim: true,
  refreshMinutes: 10,
  priorityMode: 'ending-soonest',
};

/**
 * Оркестратор приложения: авторизация, склад кампаний, PubSub и автофарм.
 * UI и CLI работают только через него.
 */
export class Engine extends Emitter {
  constructor({ dataDir, onLog, tokenCipher = null, rustWatchExe = null } = {}) {
    super();
    this.dataDir = dataDir;
    this.settingsPath = path.join(dataDir, 'settings.json');
    // кэш каталога: если источники недоступны, показываем последний удачный список
    setCatalogCache(path.join(dataDir, 'catalog-cache.json'));
    this.settings = { ...DEFAULT_SETTINGS };
    this.authStore = new AuthStore(path.join(dataDir, 'auth.json'), tokenCipher);
    this.gql = new GqlClient({ profile: CLIENTS.web });
    this.api = new TwitchApi(this.gql);
    this.store = new CampaignStore(this.api);
    this.session = null;
    this.user = null;
    this.pubsub = new PubSub({
      getToken: () => this.session?.token,
      getTopics: () => (this.session?.userId ? [TOPIC.userDrops(this.session.userId)] : []),
      onAuthError: () => this.log('PubSub: токен отклонён, нужен повторный вход', 'error'),
    });
    this.watcher = new Watcher({
      api: this.api,
      store: this.store,
      settings: this.settings,
      session: this.session,
      rustWatchExe,
    });
    this.status = { state: 'idle', running: false, message: 'Готов к работе', channel: null, game: null };
    /** PubSub может жить в отдельном Rust-сервисе: учитываем и его состояние. */
    this.rustPubsubConnected = false;
    this.logs = [];
    this.lastRefresh = null;
    this.refreshing = false;
    this.starting = false;
    this.claiming = new Set();
    this.refreshTimer = null;
    this.onLog = onLog;
    this.#wireEvents();
  }

  get loggedIn() {
    return Boolean(this.session?.token && this.user);
  }

  #wireEvents() {
    this.pubsub.on('status', (status) => {
      if (status.connecting) this.emit('pubsub', { status: 'connecting' });
      else if (status.connected) this.emit('pubsub', { status: 'online' });
      else this.emit('pubsub', { status: 'offline' });
    });
    this.pubsub.on('error', (message) => this.log(message, 'warn'));
    this.pubsub.on('disabled', () =>
      this.log('Живые события Twitch недоступны для этого токена — прогресс и награды идут опросом', 'warn'),
    );
    this.pubsub.on('message', ({ topic, payload }) => this.#handlePubSub(topic, payload));

    this.watcher.on('status', (status) => {
      this.status = {
        state: status.state,
        running: status.running,
        message: status.message,
        channel: status.channel,
        game: status.game,
        campaignId: status.campaignId,
      };
      this.emit('status', this.status);
      this.#broadcast();
    });
    this.watcher.on('error', (message) => this.log(message, 'error'));
    this.watcher.on('progress', () => this.#broadcast());
  }

  log(message, level = 'info') {
    const entry = { at: new Date().toISOString(), level, message };
    this.logs.push(entry);
    if (this.logs.length > 400) this.logs.shift();
    this.emit('log', entry);
    this.onLog?.(entry);
  }

  async init() {
    await mkdir(this.dataDir, { recursive: true });
    await this.#loadSettings();
    const session = await this.authStore.load();
    if (session?.token) {
      try {
        await this.#applySession(session);
      } catch (error) {
        this.log(`Сессия недействительна: ${error.message}`, 'warn');
        await this.authStore.clear();
        this.session = null;
        this.user = null;
      }
    }
    this.emit('auth', this.authSnapshot());
    return this.loggedIn;
  }

  async #loadSettings() {
    try {
      const raw = JSON.parse(await readFile(this.settingsPath, 'utf8'));
      this.settings = { ...DEFAULT_SETTINGS, ...raw, games: raw.games ?? [] };
    } catch {
      this.settings = { ...DEFAULT_SETTINGS };
    }
    this.watcher.settings = this.settings;
  }

  async saveSettings(patch = {}) {
    this.settings = { ...this.settings, ...patch };
    this.watcher.settings = this.settings;
    await mkdir(this.dataDir, { recursive: true });
    const tmpPath = `${this.settingsPath}.tmp`;
    await writeFile(tmpPath, JSON.stringify(this.settings, null, 2), 'utf8');
    await rename(tmpPath, this.settingsPath);
    this.emit('settings', this.settings);
    return this.settings;
  }

  async #applySession({ token }) {
    const info = await validateToken(token);
    const profile = profileForClientId(info.clientId);
    if (profile.key === 'mobileWeb') {
      throw new Error(
        'Twitch выдал токен клиента, которому закрыт GraphQL. Войдите заново — приложение запросит доступ у другого клиента.',
      );
    }
    this.gql.setProfile(profile);
    this.gql.setToken(token);
    this.session = {
      token,
      userId: info.userId,
      login: info.login,
      clientId: info.clientId,
      profileKey: profile.key,
      userAgent: profile.userAgent,
      savedAt: new Date().toISOString(),
    };
    this.user = { id: info.userId, login: info.login, displayName: info.login };
    this.store.setLogin(info.login);
    this.watcher.session = this.session;
    this.profileLabel = profile.label;
    await this.authStore.save(this.session);
    this.pubsub.stop();
    this.pubsub.setTopics([TOPIC.userDrops(info.userId)]);
    this.pubsub.start();
    this.emit('auth', this.authSnapshot());
    return this.session;
  }

  /** Вход по готовому OAuth-токену (например, из cookie `auth-token`). */
  async loginWithToken(token) {
    const clean = String(token ?? '').trim().replace(/^oauth:/i, '');
    if (!clean) throw new Error('Токен пустой');
    this.log('Проверяю токен…');
    await this.#applySession({ token: clean });
    this.log(`Вход выполнен: ${this.user.login} (${this.profileLabel ?? 'Twitch'})`);
    try {
      await this.refresh({ silent: true });
    } catch (error) {
      if (error?.code === 'UNAUTHORIZED' || /401|недействителен/i.test(error?.message ?? '')) {
        throw new Error(
          'Twitch не принимает этот токен в GraphQL. Войдите через «Войти в окне приложения» — оно берёт токен веб-клиента.',
        );
      }
      throw error;
    }
    return this.authSnapshot();
  }

  /** Вход через браузер: пользователь подтверждает код на twitch.tv/activate. */
  async loginWithDeviceCode(onCode, { signal } = {}) {
    const { token } = await deviceCodeLogin({ onCode, signal });
    return this.loginWithToken(token);
  }

  async logout() {
    await this.watcher.stop();
    clearTimeout(this.refreshTimer);
    this.refreshTimer = null;
    this.#stopClaimWatch();
    this.pubsub.stop();
    this.pubsub.setTopics([]);
    await this.authStore.clear();
    this.session = null;
    this.user = null;
    this.gql.setToken(null);
    this.store.campaigns = [];
    this.store.byId.clear();
    this.store.byDropId.clear();
    this.emit('auth', this.authSnapshot());
    this.#broadcast();
  }

  authSnapshot() {
    return {
      loggedIn: this.loggedIn,
      user: this.user,
      profile: this.session?.profileKey ?? null,
      tokenPreview: this.session?.token ? `${this.session.token.slice(0, 4)}…${this.session.token.slice(-4)}` : null,
    };
  }

  async refresh({ silent = false } = {}) {
    if (!this.loggedIn) throw new Error('Нужен вход в Twitch');
    if (this.refreshing) return this.refreshPromise ?? this.store.campaigns;

    this.refreshing = true;
    const task = (async () => {
      try {
        if (!silent) this.emit('refresh', { active: true, message: 'Обновляю кампании…' });
        const campaigns = await this.store.refresh({
          focus: [...(this.settings.games ?? []), ...(this.settings.pinned ?? [])],
          onProgress: (progress) => {
            if (!silent) this.emit('refresh', { active: true, ...progress });
          },
        });
        this.lastRefresh = new Date();
        this.log(`Загружено кампаний: ${campaigns.length}, дропов: ${this.store.byDropId.size}`);
        if (this.settings.autoClaim) await this.claimAll();
        if (!silent) this.emit('refresh', { active: false });
        this.#broadcast();
        this.#scheduleRefresh();
        return campaigns;
      } finally {
        this.refreshing = false;
        this.refreshPromise = null;
      }
    })();

    this.refreshPromise = task;
    return task;
  }

  #scheduleRefresh() {
    clearTimeout(this.refreshTimer);
    const minutes = clamp(Number(this.settings.refreshMinutes) || 10, 1, 120);
    this.refreshTimer = setTimeout(() => {
      this.refresh({ silent: true }).catch((error) => this.log(error.message, 'error'));
    }, minutes * 60 * 1000);
  }

  /**
   * Кнопка «Фармить» в карточке игры: добавляет игру в список и сразу
   * запускает автофарм, если он ещё не идёт.
   */
  async farmGame(slug) {
    if (!this.loggedIn) throw new Error('Сначала войдите в Twitch');
    const games = new Set(this.settings.games);
    games.add(slug);
    await this.saveSettings({ games: [...games] });

    if (this.watcher.running) {
      this.watcher.setTargets([...games]);
      this.log(`Игра добавлена в фарм: ${slug}`);
    } else {
      await this.startFarming();
    }
    this.#broadcast();
    return this.snapshot();
  }

  /** Закрепить/открепить игру: закреплённые идут первыми в списке. */
  async togglePin(slug) {
    const pinned = new Set(this.settings.pinned ?? []);
    if (pinned.has(slug)) pinned.delete(slug);
    else pinned.add(slug);
    await this.saveSettings({ pinned: [...pinned] });
    this.#broadcast();
    return this.snapshot();
  }

  /** Убрать все игры из очереди и остановить фарм, если он шёл. */
  async clearQueue() {
    await this.saveSettings({ games: [] });
    if (this.watcher.running) await this.stopFarming();
    this.log('Очередь очищена');
    this.#broadcast();
    return this.snapshot();
  }

  async startFarming() {
    if (!this.loggedIn) throw new Error('Сначала войдите в Twitch');
    if (!this.settings.games.length) throw new Error('Выберите хотя бы одну игру');
    if (this.starting) return;
    this.starting = true;
    try {
      if (!this.store.campaigns.length) await this.refresh({ silent: true });
      this.watcher.setTargets(this.settings.games);
      this.watcher.start();
      this.#startClaimWatch();
      this.log('Автофарм запущен');
      this.#broadcast();
    } finally {
      this.starting = false;
    }
  }

  async stopFarming() {
    await this.watcher.stop();
    this.#stopClaimWatch();
    this.log('Автофарм остановлен');
    this.#broadcast();
  }

  async toggleGame(slug) {
    const games = new Set(this.settings.games);
    if (games.has(slug)) games.delete(slug);
    else games.add(slug);
    await this.saveSettings({ games: [...games] });
    if (this.watcher.running) this.watcher.setTargets(this.settings.games);
    this.#broadcast();
    return [...games];
  }

  async selectOnly(slugs) {
    await this.saveSettings({ games: [...new Set(slugs)] });
    if (this.watcher.running) this.watcher.setTargets(this.settings.games);
    this.#broadcast();
  }

  /**
   * PubSub может быть недоступен для токена, выданного не тому клиенту —
   * тогда прогресс идёт только опросом, и награды забираем по таймеру.
   */
  #startClaimWatch() {
    this.#stopClaimWatch();
    this.claimTimer = setInterval(() => {
      if (this.settings.autoClaim) this.claimAll().catch(() => {});
    }, 120_000);
  }

  #stopClaimWatch() {
    clearInterval(this.claimTimer);
    this.claimTimer = null;
  }

  async claimAll() {
    if (!this.loggedIn) return 0;
    const pending = this.store.claimableDrops().filter(({ drop }) => drop.claimId);
    let claimed = 0;
    for (const { drop } of pending) {
      if (this.claiming.has(drop.id)) continue;
      this.claiming.add(drop.id);
      try {
        const ok = await this.api.claimDrop(drop.claimId);
        if (ok) {
          this.store.markClaimed(drop.id);
          claimed += 1;
          const campaign = this.store.findCampaign(drop.campaignId);
          this.log(`Награда получена: ${drop.name} (${campaign?.game.name ?? ''})`, 'success');
        }
      } catch (error) {
        this.log(`Не удалось забрать «${drop.name}»: ${error.message}`, 'warn');
      } finally {
        this.claiming.delete(drop.id);
      }
    }
    if (claimed) this.#broadcast();
    return claimed;
  }

  async claimDropInstance(dropId, instanceId) {
    this.store.setClaimId(dropId, instanceId);
    const found = this.store.findDrop(dropId);
    if (!found) return false;
    try {
      const ok = await this.api.claimDrop(instanceId);
      if (ok) {
        this.store.markClaimed(dropId);
        this.log(`Награда получена: ${found.drop.name}`, 'success');
        this.#broadcast();
      }
      return ok;
    } catch (error) {
      this.log(`Ошибка получения награды: ${error.message}`, 'warn');
      return false;
    }
  }

  /** Событие из внешнего PubSub-сервиса (Rust): разбирается так же, как своё. */
  applyPubSubMessage(topic, payload) {
    this.#handlePubSub(topic, payload);
  }

  #handlePubSub(topic, payload) {
    if (!payload || typeof payload !== 'object') return;
    if (payload.type === 'drop-progress') {
      const { drop_id: dropId, current_progress_min: current, required_progress_min: required } =
        payload.data ?? {};
      if (!dropId) return;
      const found = this.store.findDrop(dropId);
      if (!found) return;
      const minutes = Number(current ?? 0);
      if (this.store.updateProgress(dropId, minutes)) {
        this.emit('progress', {
          campaignId: found.campaign.id,
          dropId,
          dropName: found.drop.name,
          currentMinutes: minutes,
          requiredMinutes: Number(required ?? found.drop.requiredMinutes),
        });
        this.#broadcast();
      }
      if (this.settings.autoClaim && minutes >= (required ?? found.drop.requiredMinutes)) {
        this.claimAll().catch(() => {});
      }
      return;
    }

    if (payload.type === 'drop-claim') {
      const { drop_id: dropId, drop_instance_id: instanceId } = payload.data ?? {};
      if (!dropId) return;
      this.log(`Twitch выдал награду за «${this.store.findDrop(dropId)?.drop.name ?? dropId}»`);
      this.claimDropInstance(dropId, instanceId)
        .then(() => this.#scheduleRefresh())
        .catch(() => {});
    }
  }

  #broadcast() {
    this.emit('state', this.snapshot());
  }

  /** Полный снимок состояния для UI (только простые данные). */
  snapshot() {
    const selected = new Set(this.settings.games);
    const campaigns = this.store.campaigns.map((campaign) => ({
      id: campaign.id,
      name: campaign.name,
      status: campaign.status,
      game: campaign.game,
      startsAt: campaign.startsAt?.toISOString() ?? null,
      endsAt: campaign.endsAt?.toISOString() ?? null,
      progress: campaign.progress,
      totalRequiredMinutes: campaign.totalRequiredMinutes,
      active: campaign.active,
      upcoming: campaign.upcoming,
      expired: campaign.expired,
      completed: campaign.completed,
      canEarn: campaign.canEarn,
      linked: campaign.linked,
      reward: campaign.reward,
      webUrl: campaign.webUrl ?? null,
      selected: selected.has(campaign.game.slug),
      farming: this.watcher.campaign?.id === campaign.id,
      allowedChannelCount: campaign.allowedChannels?.length ?? 0,
      drops: campaign.drops.map((drop) => ({
        id: drop.id,
        name: drop.name,
        currentMinutes: drop.currentMinutes,
        requiredMinutes: drop.requiredMinutes,
        remainingMinutes: drop.remainingMinutes,
        progress: drop.progress,
        isClaimed: drop.isClaimed,
        canClaim: drop.canClaim,
        startsAt: drop.startsAt?.toISOString() ?? null,
        endsAt: drop.endsAt?.toISOString() ?? null,
        benefits: drop.benefits,
        reward: campaign.reward,
      })),
    }));

    const pinnedGames = new Set(this.settings.pinned ?? []);
    const games = new Map();
    for (const campaign of this.store.campaigns) {
      const key = campaign.game.slug;
      if (!key) continue;
      if (!games.has(key)) {
        games.set(key, {
          slug: key,
          name: campaign.game.name,
          boxArtUrl: campaign.game.boxArtUrl,
          campaignIds: [],
          totalDrops: 0,
          claimedDrops: 0,
          endsAt: null,
          active: false,
          canEarn: false,
          campaignCount: 0,
          rewardCount: 0,
        });
      }
      const game = games.get(key);
      game.campaignIds.push(campaign.id);
      game.campaignCount += 1;
      if (campaign.reward) game.rewardCount += 1;
      game.active ||= campaign.active;
      game.canEarn ||= campaign.canEarn;
      // прогресс и срок считаем по живым кампаниям: завершённые только шумят
      if (campaign.expired) continue;
      game.totalDrops += campaign.drops.length;
      game.claimedDrops += campaign.drops.filter((drop) => drop.isClaimed).length;
      if (campaign.endsAt && (!game.endsAt || campaign.endsAt < new Date(game.endsAt))) {
        game.endsAt = campaign.endsAt.toISOString();
      }
    }

    const allDrops = this.store.campaigns.flatMap((campaign) => campaign.drops);
    return {
      ready: true,
      auth: this.authSnapshot(),
      status: this.status,
      pubsub: this.pubsub.connected || this.rustPubsubConnected ? 'online' : 'offline',
      lastRefresh: this.lastRefresh?.toISOString() ?? null,
      catalog: this.store.catalog ?? null,
      settings: this.settings,
      stats: {
        games: games.size,
        campaigns: campaigns.length,
        drops: allDrops.length,
        claimed: allDrops.filter((drop) => drop.isClaimed).length,
        claimable: allDrops.filter((drop) => drop.canClaim).length,
        minutesEarned: allDrops.reduce(
          (sum, drop) => sum + (drop.isClaimed ? drop.requiredMinutes : drop.currentMinutes),
          0,
        ),
      },
      games: [...games.values()].map(({ campaignCount, rewardCount, ...game }) => ({
        ...game,
        rewardOnly: campaignCount > 0 && rewardCount === campaignCount,
        selected: selected.has(game.slug),
        pinned: pinnedGames.has(game.slug),
        farming: this.watcher.campaign?.game.slug === game.slug,
        endsAt: game.endsAt,
      })),
      campaigns,
      logs: this.logs.slice(-120),
    };
  }
}
