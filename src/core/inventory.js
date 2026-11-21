import { hasRustCore, rustCore } from './native.js';
import { fetchCatalog, catalogStatus } from './catalog.js';
import { buildCampaign, groupByGame, mergeData } from './model.js';

const DETAIL_CHUNK = 10;

/**
 * Какие игры стоит перепроверить в Twitch: всё уже закончилось или игра
 * в очереди/закреплена. Чистая функция — её же проверяет Rust-порт.
 */
export function selectStaleGames(candidates, focus = [], options = {}) {
  const { now = Date.now(), cooldowns = {}, limit = 6 } = options;
  if (hasRustCore && rustCore.inventory) {
    return rustCore.inventory.selectStaleGames(candidates, focus, { now, cooldowns, limit });
  }
  const byGame = new Map();

  for (const campaign of candidates) {
    const slug = campaign?.game?.slug;
    if (!slug) continue;
    const end = campaign.endAt ? new Date(campaign.endAt).getTime() : Infinity;
    const entry = byGame.get(slug) ?? { slug, latestEnd: 0 };
    if (end > entry.latestEnd) entry.latestEnd = end;
    byGame.set(slug, entry);
  }

  const wanted = new Set(focus);
  const slugs = [];
  for (const [slug, entry] of byGame) {
    const ended = !Number.isFinite(entry.latestEnd) || entry.latestEnd < now;
    if (ended || wanted.has(slug)) slugs.push(slug);
  }
  if (!slugs.length) return [];

  const ready = [...new Set(slugs)].filter((slug) => (cooldowns[slug] ?? 0) < now);
  return ready.slice(0, limit);
}



/**
 * Хранилище кампаний: загрузка, слияние данных, локальные обновления прогресса.
 */
export class CampaignStore {
  constructor(api, { login } = {}) {
    this.api = api;
    this.login = login;
    this.campaigns = [];
    this.byId = new Map();
    this.byDropId = new Map();
    this.loadedAt = null;
    /** Состояние источника каталога: ok / stale / source / at / message. */
    this.catalog = null;
  }

  setLogin(login) {
    this.login = login;
  }

  get games() {
    return groupByGame(this.campaigns);
  }

  get activeCampaigns() {
    return this.campaigns.filter((campaign) => !campaign.expired);
  }

  async refresh({ onProgress, focus = [] } = {}) {
    onProgress?.({ phase: 'campaigns', message: 'Получаю список кампаний…' });
    const [catalog, dashboard, inventory] = await Promise.all([
      fetchCatalog().catch((error) => {
        onProgress?.({ phase: 'campaigns', message: `Каталог недоступен: ${error.message}` });
        return [];
      }),
      this.api.getCampaigns().catch(() => []),
      this.api.getInventory().catch(() => ({ inProgress: [], claimedBenefits: [] })),
    ]);

    this.catalog = catalogStatus();
    if (!this.catalog.ok) {
      onProgress?.({
        phase: 'campaigns',
        message: 'Список кампаний недоступен — показываю только ваши активные кампании',
      });
    } else if (this.catalog.stale) {
      onProgress?.({
        phase: 'campaigns',
        message: `Источники каталога недоступны — использую кэш от ${this.catalog.at ?? 'прошлого запуска'}`,
      });
    }

    const claimedBenefits = new Map(
      (inventory.claimedBenefits ?? []).map((benefit) => [
        benefit.id,
        benefit.lastAwardedAt ? new Date(benefit.lastAwardedAt) : new Date(0),
      ]),
    );

    // Каталог даёт полный список игр, инвентарь — прогресс по моим кампаниям.
    const candidates = new Map();
    for (const campaign of catalog) {
      if (campaign?.id) candidates.set(campaign.id, campaign);
    }
    for (const campaign of [...(inventory.inProgress ?? []), ...dashboard]) {
      if (!campaign?.id) continue;
      if (campaign.status && campaign.status === 'EXPIRED') continue;
      candidates.set(campaign.id, mergeData(campaign, candidates.get(campaign.id) ?? null));
    }

    // Публичный каталог обновляется с задержкой и легко пропускает кампанию,
    // которая стартовала час назад. Поэтому игры из очереди и игры, у которых
    // всё уже закончилось, добираем напрямую из Twitch: у любого живого канала
    // с Drops есть список доступных ему кампаний.
    await this.#refreshStaleGames(candidates, focus, onProgress);

    // Детали нужны только там, где у аккаунта есть прогресс: для остальных
    // кампаний каталог уже содержит всё необходимое.
    const ids = [...candidates.values()]
      .filter((campaign) => campaign.self || campaign.timeBasedDrops?.some((drop) => drop.self))
      .map((campaign) => campaign.id);
    onProgress?.({
      phase: 'details',
      message: `Загружаю детали кампаний (${ids.length})…`,
      total: ids.length,
      done: 0,
    });

    const details = new Map();
    let done = 0;
    for (let offset = 0; offset < ids.length; offset += DETAIL_CHUNK) {
      const chunk = ids.slice(offset, offset + DETAIL_CHUNK);
      const operations = chunk.map((id) => ({
        name: 'DropCampaignDetails',
        variables: { channelLogin: this.login, dropID: id },
      }));
      const results = await this.api.gql.batch(operations).catch((error) => {
        onProgress?.({ phase: 'details', message: `Ошибка деталей: ${error.message}` });
        return [];
      });
      results.forEach((data, index) => {
        const campaign = data?.user?.dropCampaign;
        if (campaign?.id) details.set(campaign.id, campaign);
      });
      done += chunk.length;
      onProgress?.({ phase: 'details', done, total: ids.length, message: 'Детали кампаний…' });
    }

    const campaigns = [];
    for (const [id, base] of candidates) {
      const merged = mergeData(details.get(id) ?? null, base);
      if (!merged?.game) continue;
      try {
        campaigns.push(buildCampaign(merged, claimedBenefits));
      } catch (error) {
        console.warn(`Кампания ${id} пропущена: ${error.message}`);
      }
    }

    campaigns.sort((a, b) => {
      const rank = (campaign) => (campaign.active ? 0 : campaign.upcoming ? 1 : 2);
      return rank(a) - rank(b) || (a.endsAt?.getTime() ?? 0) - (b.endsAt?.getTime() ?? 0);
    });

    this.campaigns = campaigns;
    this.byId = new Map(campaigns.map((campaign) => [campaign.id, campaign]));
    this.byDropId = new Map();
    for (const campaign of campaigns) {
      for (const drop of campaign.drops) this.byDropId.set(drop.id, { campaign, drop });
    }
    this.loadedAt = new Date();
    return campaigns;
  }

  /**
   * Добирает кампании напрямую из Twitch для игр, где каталог может отставать:
   * всё уже закончилось (значит, могла начаться новая) или игра в очереди.
   */
  async #refreshStaleGames(candidates, focus, onProgress) {
    const cooldown = this.probeCooldown ?? (this.probeCooldown = new Map());
    const byGame = new Map();
    for (const campaign of candidates.values()) {
      const slug = campaign?.game?.slug;
      if (!slug) continue;
      const existing = byGame.get(slug) ?? { slug, game: campaign.game };
      byGame.set(slug, existing);
    }

    const limited = selectStaleGames([...candidates.values()], focus, {
      now: Date.now(),
      cooldowns: Object.fromEntries(cooldown),
      limit: 6,
    });
    if (!limited.length) return;
    onProgress?.({ phase: 'fresh', message: `Проверяю новые кампании (${limited.length})…` });

    const queue = [...limited];
    const workers = Array.from({ length: Math.min(3, queue.length) }, async () => {
      while (queue.length) {
        const slug = queue.shift();
        const entry = byGame.get(slug);
        try {
          const campaigns = await this.#campaignsForGame(slug);
          if (!campaigns.length) cooldown.set(slug, Date.now() + 5 * 60 * 1000);
          for (const campaign of campaigns) {
            const raw = rawFromAvailableDrops(campaign, entry.game);
            if (!raw) continue;
            candidates.set(raw.id, mergeData(candidates.get(raw.id), raw));
          }
        } catch {
          /* сеть может отказать — каталог остаётся основным источником */
        }
      }
    });
    await Promise.all(workers);
  }

  /** Кампании игры по данным Twitch: идём на любой живой канал с Drops. */
  async #campaignsForGame(slug) {
    const streams = await this.api.getGameStreams(slug, { limit: 5, dropsEnabled: true });
    for (const stream of streams) {
      const channelId = stream?.broadcaster?.id;
      if (!channelId) continue;
      const campaigns = await this.api.getAvailableDrops(channelId);
      if (campaigns.length) return campaigns;
    }
    return [];
  }

  findCampaign(id) {
    return this.byId.get(id) ?? null;
  }

  findDrop(dropId) {
    return this.byDropId.get(dropId) ?? null;
  }

  /** Обновляет прогресс дропа по данным PubSub/GQL. Возвращает true, если стало больше. */
  updateProgress(dropId, minutes) {
    const found = this.byDropId.get(dropId);
    if (!found) return false;
    const { drop } = found;
    if (drop.isClaimed) return false;
    if (minutes <= drop.currentMinutes) return false;
    drop.currentMinutes = Math.min(minutes, drop.requiredMinutes);
    return true;
  }

  markClaimed(dropId) {
    const found = this.byDropId.get(dropId);
    if (!found) return false;
    found.drop.isClaimed = true;
    found.drop.currentMinutes = found.drop.requiredMinutes;
    return true;
  }

  setClaimId(dropId, instanceId) {
    const found = this.byDropId.get(dropId);
    if (!found) return false;
    found.drop.claimId = instanceId;
    return true;
  }

  /** Все дропы, которые доросли до 100% и ещё не получены. */
  claimableDrops() {
    return this.campaigns.flatMap((campaign) =>
      campaign.drops.filter((drop) => drop.canClaim).map((drop) => ({ campaign, drop })),
    );
  }

  /** Кампании, которые пользователь выбрал для фарма. */
  selectedCampaigns(slugs) {
    const wanted = new Set(slugs);
    return this.campaigns.filter(
      (campaign) => wanted.has(campaign.game.slug) && campaign.canEarn,
    );
  }
}

/** Приводит кампанию из AvailableDrops к «сырому» виду каталога. */
function rawFromAvailableDrops(campaign, fallbackGame) {
  if (!campaign?.id) return null;
  const drops = (campaign.timeBasedDrops ?? []).filter((drop) => drop?.id);
  const starts = drops
    .map((drop) => drop.startAt)
    .filter(Boolean)
    .sort();

  return {
    id: campaign.id,
    name: campaign.name,
    status: campaign.status ?? 'ACTIVE',
    startAt: starts[0] ?? null,
    endAt: campaign.endAt ?? null,
    accountLinkURL: null,
    game: {
      id: campaign.game?.id ?? fallbackGame?.id ?? null,
      name: campaign.game?.name ?? fallbackGame?.name ?? 'Неизвестная игра',
      slug: fallbackGame?.slug ?? '',
      boxArtURL: fallbackGame?.boxArtURL ?? null,
    },
    allow: { isEnabled: false, channels: [] },
    timeBasedDrops: drops.map((drop, index) => ({
      id: drop.id,
      name: drop.name ?? `Награда ${index + 1}`,
      startAt: drop.startAt,
      endAt: drop.endAt,
      requiredMinutesWatched: drop.requiredMinutesWatched,
      preconditionDrops: [],
      benefitEdges: (drop.benefitEdges ?? []).map((edge) => ({
        benefit: {
          id: edge.benefit?.id ?? null,
          name: edge.benefit?.name ?? 'Награда',
          distributionType: 'DIRECT_ENTITLEMENT',
          imageAssetURL: edge.benefit?.imageAssetURL ?? null,
        },
      })),
    })),
    suggestedChannel: null,
    source: 'twitch',
  };
}
