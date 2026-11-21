import { hasRustCore, rustCore } from './native.js';
import { readFile, writeFile, mkdir, rename } from 'node:fs/promises';
import path from 'node:path';
import { request } from './http.js';

/**
 * Каталог активных Twitch Drops.
 *
 * Twitch отдаёт список кампаний только веб-клиенту с integrity-токеном,
 * который считает JS самой страницы, поэтому каталог берём из публичного
 * источника. Раньше это был сервис Gear Up, но он сломался: отвечает
 * `HTTP 200` с заголовками и никогда не присылает тело, из-за чего загрузка
 * списка висела до таймаута. Теперь источники — открытый API
 * drops.sunkwi.com и его снимок в GitHub (обновляется каждые 10 минут,
 * без лимитов и ключей). Прогресс по кампаниям всё равно берётся из твоего
 * аккаунта (Inventory).
 *
 * Кроме игровых Drops у Twitch есть «кампании-награды» (reward campaigns,
 * например GTAVI Launch Celebration): там награда выдаётся за просмотр любой
 * категории и забирается на twitch.tv/drops/rewards, а не за прогресс дропа.
 * Их тоже показываем в списке, но не ставим в автофарм — иначе приложение
 * пыталось бы качать часы по механике, которой у этой кампании нет.
 *
 * Если все источники недоступны, отдаём последний удачный каталог с диска,
 * чтобы список не пропадал при кратковременных сбоях.
 */
const DROP_SOURCES = [
  {
    key: 'snapshot',
    label: 'снимок drops.sunkwi.com',
    url: 'https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/drops.json',
  },
  {
    key: 'api',
    label: 'drops.sunkwi.com',
    url: 'https://drops.sunkwi.com/api/twitch/drops',
  },
];

const REWARD_SOURCES = [
  {
    key: 'api',
    label: 'награды drops.sunkwi.com',
    url: 'https://drops.sunkwi.com/api/twitch/rewards',
  },
  {
    key: 'snapshot',
    label: 'снимок наград drops.sunkwi.com',
    url: 'https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/rewards.json',
  },
];

const REWARDS_PAGE = 'https://www.twitch.tv/drops/rewards';

const USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

const DEFAULT_TIMEOUT_MS = 8000;

let cacheFile = null;
let state = {
  ok: false,
  stale: false,
  source: null,
  at: null,
  message: null,
};

/** Путь к файлу кэша каталога (рядом с настройками приложения). */
export function setCatalogCache(filePath) {
  cacheFile = filePath || null;
}

/** Состояние последней загрузки каталога — для журнала и UI. */
export function catalogStatus() {
  return { ...state };
}

const normalizeBoxArt = (url) => {
  if (!url) return null;
  return String(url).replace(/-\d+x\d+(?=\.\w+$)/, '-285x380');
};

function toRawCampaign(group, campaign) {
  const game = campaign.game ?? {};
  const allow = campaign.allow ?? {};
  return {
    id: campaign.id,
    name: campaign.name,
    status: campaign.status ?? 'ACTIVE',
    startAt: campaign.startAt,
    endAt: campaign.endAt,
    accountLinkURL: campaign.accountLinkURL ?? null,
    game: {
      id: game.id ?? group.gameId,
      name: game.displayName ?? game.name ?? group.gameDisplayName ?? 'Неизвестная игра',
      slug: game.slug ?? '',
      boxArtURL: normalizeBoxArt(group.gameBoxArtURL),
    },
    allow: {
      isEnabled: allow.isEnabled === true,
      channels: (allow.channels ?? []).map((channel) => ({
        id: channel.id,
        name: channel.displayName ?? channel.name,
      })),
    },
    timeBasedDrops: (campaign.timeBasedDrops ?? []).map((drop, index) => ({
      id: drop.id,
      name: drop.name ?? `Награда ${index + 1}`,
      startAt: drop.startAt,
      endAt: drop.endAt,
      requiredMinutesWatched: drop.requiredMinutesWatched,
      preconditionDrops: [],
      benefitEdges: (drop.benefitEdges ?? []).map((edge, benefitIndex) => ({
        benefit: {
          id: edge.benefit?.id ?? `${drop.id}-${benefitIndex}`,
          name: edge.benefit?.name ?? 'Награда',
          distributionType: edge.benefit?.distributionType ?? 'DIRECT_ENTITLEMENT',
          imageAssetURL: edge.benefit?.imageAssetURL ?? null,
        },
      })),
    })),
    suggestedChannel: null,
    source: 'catalog',
  };
}

/**
 * «Кампания-награда»: награда выдаётся за просмотр любой категории.
 * В дроп-модель переносим только группы, где нужны минуты просмотра:
 * награды за подписку автофармом не получить.
 */
function toRewardCampaign(campaign) {
  const groups = campaign.rewardGroups ?? [];
  const category = groups[0]?.categories?.[0] ?? campaign.game ?? {};
  const drops = [];

  for (const group of groups) {
    const minutes = Number(group.unlockRequirements?.minuteWatchedGoal ?? 0);
    if (!minutes) continue;
    for (const reward of group.rewards ?? []) {
      if (!reward?.id) continue;
      drops.push({
        id: reward.id,
        name: reward.name ?? 'Награда',
        startAt: campaign.startsAt,
        endAt: reward.earnableUntil ?? campaign.endsAt,
        requiredMinutesWatched: minutes,
        preconditionDrops: [],
        benefitEdges: [
          {
            benefit: {
              id: reward.id,
              name: reward.name ?? 'Награда',
              distributionType: 'QUEST_REWARD',
              imageAssetURL: reward.thumbnailImage?.image1xURL ?? null,
            },
          },
        ],
      });
    }
  }

  return {
    id: campaign.id,
    name: campaign.name,
    status: campaign.status ?? 'ACTIVE',
    startAt: campaign.startsAt,
    endAt: campaign.endsAt,
    accountLinkURL: null,
    game: {
      id: category.id ?? null,
      name: category.displayName ?? category.name ?? 'Twitch',
      slug: category.slug ?? '',
      // обложку для кампаний-наград берём по id игры из того же CDN, что и Twitch
      boxArtURL: category.id
        ? `https://static-cdn.jtvnw.net/ttv-boxart/${category.id}-285x380.jpg`
        : null,
    },
    allow: { isEnabled: false, channels: [] },
    timeBasedDrops: drops,
    rewardCampaign: true,
    webUrl: campaign.webUrl ?? REWARDS_PAGE,
    suggestedChannel: null,
    source: 'reward',
  };
}

/** Разбирает ответ drops.sunkwi.com по игровым дропам: игры, внутри — кампании. */
export function parseDropPayload(payload) {
  if (hasRustCore) return rustCore.catalog.parseDropPayload(payload);
  const games = payload?.games;
  if (!Array.isArray(games)) throw new Error('неожиданный формат ответа');

  const campaigns = [];
  for (const group of games) {
    for (const campaign of group.rewards ?? []) {
      if (!campaign?.id) continue;
      if (campaign.status === 'EXPIRED') continue;
      campaigns.push(toRawCampaign(group, campaign));
    }
  }
  return campaigns;
}

/** Разбирает ответ по кампаниям-наградам (reward campaigns). */
export function parseRewardPayload(payload) {
  if (hasRustCore) return rustCore.catalog.parseRewardPayload(payload);
  const campaigns = payload?.campaigns;
  if (!Array.isArray(campaigns)) throw new Error('неожиданный формат ответа');

  return campaigns
    .filter((campaign) => campaign?.id && campaign.status !== 'EXPIRED')
    .map(toRewardCampaign)
    .filter((campaign) => campaign.game.slug && campaign.timeBasedDrops.length);
}

async function loadFirst(sources, parse, errors, timeoutMs) {
  for (const source of sources) {
    try {
      const response = await request(source.url, {
        headers: { 'User-Agent': USER_AGENT, Accept: 'application/json' },
        parse: 'json',
        retries: 0,
        timeoutMs,
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const campaigns = parse(response.data);
      if (!campaigns.length) throw new Error('пустой список');
      return { source, campaigns };
    } catch (error) {
      errors.push(`${source.label}: ${error.message}`);
    }
  }
  return null;
}

async function readCache() {
  if (!cacheFile) return null;
  try {
    const raw = JSON.parse(await readFile(cacheFile, 'utf8'));
    return Array.isArray(raw?.campaigns) && raw.campaigns.length ? raw : null;
  } catch {
    return null;
  }
}

async function writeCache(campaigns) {
  if (!cacheFile) return;
  try {
    await mkdir(path.dirname(cacheFile), { recursive: true });
    const payload = { at: new Date().toISOString(), campaigns };
    const tmpPath = `${cacheFile}.tmp`;
    await writeFile(tmpPath, JSON.stringify(payload), 'utf8');
    await rename(tmpPath, cacheFile);
  } catch {
    /* кэш не критичен */
  }
}

/** Возвращает кампании в том же «сыром» виде, что и GraphQL Twitch. */
export async function fetchCatalog({ timeoutMs = DEFAULT_TIMEOUT_MS, onStatus } = {}) {
  const errors = [];
  const [drops, rewards] = await Promise.all([
    loadFirst(DROP_SOURCES, parseDropPayload, errors, timeoutMs),
    loadFirst(REWARD_SOURCES, parseRewardPayload, errors, timeoutMs),
  ]);

  const campaigns = [...(drops?.campaigns ?? []), ...(rewards?.campaigns ?? [])];

  if (campaigns.length) {
    const used = [drops, rewards].filter(Boolean).map((result) => result.source.key);
    state = {
      ok: true,
      stale: false,
      source: used.join('+'),
      at: new Date().toISOString(),
      message: errors.length ? errors.join('; ') : null,
    };
    await writeCache(campaigns);
    onStatus?.(catalogStatus());
    return campaigns;
  }

  const cached = await readCache();
  if (cached) {
    state = {
      ok: true,
      stale: true,
      source: 'cache',
      at: cached.at ?? null,
      message: errors.join('; '),
    };
    onStatus?.(catalogStatus());
    return cached.campaigns;
  }

  state = {
    ok: false,
    stale: false,
    source: null,
    at: null,
    message: errors.join('; '),
  };
  onStatus?.(catalogStatus());
  throw new Error(`Каталог недоступен (${state.message})`);
}
