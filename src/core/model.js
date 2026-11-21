import { decodeMarkers, hasRustCore, rustCore } from './native.js';
/**
 * Модели кампаний и дропов: превращают сырые ответы Twitch в удобные объекты
 * с готовыми вычислениями прогресса, сроков и доступности.
 *
 * Разделено на два шага, чтобы перенос в Rust проверялся детерминированно:
 * `campaignData()` собирает данные без обращения к часам, а `campaignView()`
 * добавляет вычисления, зависящие от текущего момента. Часы передаются
 * аргументом — снаружи их берёт `buildCampaign()`.
 */

export const DROP_STATUS = {
  ACTIVE: 'ACTIVE',
  UPCOMING: 'UPCOMING',
  EXPIRED: 'EXPIRED',
};

const stripDimensions = (url) => (url ? String(url).replace(/-?\d+x\d+(?=\.\w+$)/, '') : null);

function parseDate(value) {
  const date = value ? new Date(value) : null;
  return date && !Number.isNaN(date.getTime()) ? date : null;
}

/** Рекурсивно сливает данные: значения из `primary` приоритетнее. */
export function mergeData(primary, secondary) {
  if (!primary) return secondary;
  if (!secondary) return primary;
  if (typeof primary !== 'object' || typeof secondary !== 'object') return primary;
  if (Array.isArray(primary) || Array.isArray(secondary)) return primary;

  const merged = {};
  for (const key of new Set([...Object.keys(primary), ...Object.keys(secondary)])) {
    if (key in primary && key in secondary) {
      merged[key] =
        primary[key] && secondary[key] && typeof primary[key] === 'object' && typeof secondary[key] === 'object'
          ? mergeData(primary[key], secondary[key])
          : primary[key];
    } else if (key in primary) {
      merged[key] = primary[key];
    } else {
      merged[key] = secondary[key];
    }
  }
  return merged;
}

function buildBenefit(edge) {
  const benefit = edge?.benefit ?? {};
  return {
    id: benefit.id ?? null,
    name: benefit.name ?? 'Награда',
    type: benefit.distributionType ?? 'UNKNOWN',
    imageUrl: benefit.imageAssetURL ?? null,
  };
}

function buildDrop(campaign, data, claimedBenefits) {
  const benefits = (data.benefitEdges ?? []).map(buildBenefit);
  const self = data.self ?? null;
  const startAt = parseDate(data.startAt);
  const endAt = parseDate(data.endAt);

  const claimedByBenefits =
    !self &&
    benefits.length > 0 &&
    benefits.every((benefit) => {
      // Map в приложении, обычный объект — в тестах и записи поведения
      const awarded =
        typeof claimedBenefits?.get === 'function'
          ? claimedBenefits.get(benefit.id)
          : claimedBenefits?.[benefit.id];
      return awarded && startAt && endAt && awarded >= startAt && awarded < endAt;
    });

  const isClaimed = Boolean(self?.isClaimed ?? claimedByBenefits);
  const requiredMinutes = Number(data.requiredMinutesWatched ?? 0);
  const currentMinutes = isClaimed ? requiredMinutes : Number(self?.currentMinutesWatched ?? 0);

  return {
    id: data.id,
    name: data.name,
    campaignId: campaign.id,
    startsAt: startAt,
    endsAt: endAt,
    requiredMinutes,
    currentMinutes,
    isClaimed,
    claimId: self?.dropInstanceID ?? null,
    benefits,
    prerequisiteIds: (data.preconditionDrops ?? []).map((drop) => drop.id),
    get remainingMinutes() {
      return Math.max(0, this.requiredMinutes - this.currentMinutes);
    },
    get progress() {
      if (this.isClaimed) return 1;
      if (!this.requiredMinutes) return 0;
      return Math.min(1, this.currentMinutes / this.requiredMinutes);
    },
    get canClaim() {
      return Boolean(this.claimId) && !this.isClaimed && this.currentMinutes >= this.requiredMinutes;
    },
  };
}

/**
 * Собирает кампанию из кратких данных и подробностей — только данные,
 * без вычислений по текущему времени.
 * @param {object} raw слитые данные GraphQL
 * @param {Map<string, Date>} claimedBenefits награды, уже полученные аккаунтом
 */
export function campaignData(raw, claimedBenefits = new Map()) {
  const game = raw.game ?? {};
  const startAt = parseDate(raw.startAt);
  const endAt = parseDate(raw.endAt);
  const drops = (raw.timeBasedDrops ?? []).map((drop) => buildDrop(raw, drop, claimedBenefits));
  const allowed = raw.allow ?? {};

  const allowedChannels =
    allowed.channels?.length && allowed.isEnabled !== false
      ? allowed.channels.map((channel) => ({
          id: String(channel.id),
          login: channel.name ?? channel.login,
          displayName: channel.displayName ?? channel.name ?? channel.login,
        }))
      : null;

  return {
    id: raw.id,
    name: raw.name,
    status: raw.status ?? 'ACTIVE',
    linkUrl: raw.accountLinkURL ?? null,
    needsAccountLink: Boolean(raw.self && raw.self.isAccountConnected === false && raw.accountLinkURL),
    linked: raw.self ? raw.self.isAccountConnected !== false : true,
    startsAt: startAt,
    endsAt: endAt,
    game: {
      id: game.id ?? null,
      name: game.displayName ?? game.name ?? 'Неизвестная игра',
      slug: game.slug ?? game.name ?? '',
      boxArtUrl: stripDimensions(game.boxArtURL),
    },
    allowedChannels,
    /** канал, который рекомендуется для этой кампании (из каталога) */
    suggestedChannel: raw.suggestedChannel ?? null,
    /** «кампания-награда»: забирается на twitch.tv/drops/rewards, а не автофармом */
    reward: raw.rewardCampaign === true,
    webUrl: raw.webUrl ?? (raw.rewardCampaign ? 'https://www.twitch.tv/drops/rewards' : null),
    drops,
  };
}

/**
 * Добавляет к данным кампании вычисления, зависящие от момента `now`.
 * Часы приходят аргументом, поэтому функцию можно записать и сравнить.
 */
export function campaignView(data, now = new Date()) {
  // принимаем и Date, и ISO-строку: так функцию можно записать и сравнить
  const at = now instanceof Date ? now : new Date(now);
  const startAt = parseDate(data.startsAt);
  const endAt = parseDate(data.endsAt);
  const drops = (data.drops ?? []).map((drop) => ({
    ...drop,
    startsAt: parseDate(drop.startsAt),
    endsAt: parseDate(drop.endsAt),
  }));

  const active = data.status !== 'EXPIRED' && startAt && endAt && startAt <= at && at < endAt;
  const upcoming = data.status === 'UPCOMING' || Boolean(startAt && startAt > at);
  const expired = data.status === 'EXPIRED' || Boolean(endAt && endAt <= at);

  const byId = new Map(drops.map((drop) => [drop.id, drop]));
  const farmableDrops = drops.filter((drop) => {
    if (drop.isClaimed) return false;
    if (drop.requiredMinutes <= 0) return false;
    if (drop.startsAt && drop.startsAt > at) return false;
    if (drop.endsAt && drop.endsAt <= at) return false;
    return drop.prerequisiteIds.every((id) => byId.get(id)?.isClaimed);
  });

  return {
    ...data,
    get active() {
      return active;
    },
    get upcoming() {
      return upcoming;
    },
    get expired() {
      return expired;
    },
    get minutesLeft() {
      return endAt ? (endAt.getTime() - at.getTime()) / 60000 : Infinity;
    },
    get totalRequiredMinutes() {
      return drops.reduce((sum, drop) => sum + (drop.isClaimed ? 0 : drop.remainingMinutes), 0);
    },
    get earnedMinutes() {
      return drops.reduce((sum, drop) => sum + (drop.isClaimed ? drop.requiredMinutes : drop.currentMinutes), 0);
    },
    get totalMinutes() {
      return drops.reduce((sum, drop) => sum + drop.requiredMinutes, 0);
    },
    get progress() {
      const total = this.totalMinutes;
      return total ? this.earnedMinutes / total : 0;
    },
    get completed() {
      return drops.length > 0 && drops.every((drop) => drop.isClaimed);
    },
    /** Дропы, которые реально можно фармить прямо сейчас. */
    get farmableDrops() {
      return farmableDrops;
    },
    get canEarn() {
      // «Кампания-награда»: часы идут в любой трансляции категории, поэтому
      // её тоже можно «смотреть» — прогресс считает Twitch, награда
      // забирается на twitch.tv/drops/rewards.
      if (data.reward) return !expired && farmableDrops.length > 0;
      return !expired && farmableDrops.length > 0 && (!endAt || endAt > at);
    },
    /** Дроп, который будет копиться первым. */
    get nextDrop() {
      return [...farmableDrops].sort((a, b) => a.remainingMinutes - b.remainingMinutes)[0] ?? null;
    },
    acceptsChannel(channel) {
      if (!channel || !data.allowedChannels) return true;
      return data.allowedChannels.some((item) => item.id === String(channel.id));
    },
  };
}

/** Полная модель кампании: данные + вычисления на момент `now`. */
export function buildCampaign(raw, claimedBenefits = new Map(), now = new Date()) {
  if (hasRustCore) {
    // Map нельзя отдать через мост: переводим в объект дат
    const claims =
      claimedBenefits instanceof Map
        ? Object.fromEntries(
            [...claimedBenefits].map(([key, value]) => [
              key,
              value instanceof Date ? value.toISOString() : value,
            ]),
          )
        : claimedBenefits;
    const data = decodeMarkers(rustCore.model.campaignData(raw, claims ?? {}));
    const view = decodeMarkers(rustCore.model.campaignView(data, now instanceof Date ? now.toISOString() : now));
    // метод не пересекает мост: возвращаем реализацию из JS-модели
    view.acceptsChannel = function acceptsChannel(channel) {
      if (!channel || !this.allowedChannels) return true;
      return this.allowedChannels.some((item) => item.id === String(channel.id));
    };
    return view;
  }
  return campaignView(campaignData(raw, claimedBenefits), now);
}

/** Группирует кампании по игре — это и есть «список игр» на главном экране. */
export function groupByGame(campaigns) {
  const games = new Map();
  for (const campaign of campaigns) {
    const key = campaign.game.slug || campaign.game.name;
    if (!key) continue;
    if (!games.has(key)) {
      games.set(key, {
        slug: key,
        name: campaign.game.name,
        boxArtUrl: campaign.game.boxArtUrl,
        gameId: campaign.game.id,
        campaigns: [],
      });
    }
    games.get(key).campaigns.push(campaign);
  }
  return [...games.values()].map((game) => ({
    ...game,
    get activeCampaigns() {
      return this.campaigns.filter((campaign) => campaign.active || campaign.upcoming);
    },
    get totalDrops() {
      return this.campaigns.reduce((sum, campaign) => sum + campaign.drops.length, 0);
    },
    get claimedDrops() {
      return this.campaigns.reduce(
        (sum, campaign) => sum + campaign.drops.filter((drop) => drop.isClaimed).length,
        0,
      );
    },
    get endsAt() {
      const dates = this.campaigns
        .map((campaign) => campaign.endsAt)
        .filter(Boolean)
        .sort((a, b) => a - b);
      return dates[0] ?? null;
    },
    get canEarn() {
      return this.campaigns.some((campaign) => campaign.canEarn);
    },
  }));
}
