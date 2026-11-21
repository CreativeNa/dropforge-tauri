import { GQL_OPS, RAW_DROPS_DASHBOARD, USHER_URL } from './constants.js';
import { request } from './http.js';

/** Тонкая обёртка над Twitch GraphQL: только «сырые» данные, без моделей. */
export class TwitchApi {
  constructor(gql) {
    this.gql = gql;
  }

  /** Список кампаний, доступных аккаунту (краткие данные). */
  async getCampaigns() {
    const data = await this.gql.query('ViewerDropsDashboard');
    const persisted = data?.currentUser?.dropCampaigns;
    if (Array.isArray(persisted) && persisted.length) return persisted;

    // Twitch отдаёт пустой список по persisted-запросу, но тот же набор
    // полей работает обычным GraphQL-запросом.
    try {
      const raw = await this.gql.queryRaw('ViewerDropsDashboard', RAW_DROPS_DASHBOARD);
      const campaigns = raw?.currentUser?.dropCampaigns;
      if (Array.isArray(campaigns) && campaigns.length) return campaigns;
    } catch {
      /* остаёмся на пустом списке */
    }
    return Array.isArray(persisted) ? persisted : [];
  }

  /** Кампании в процессе + уже полученные награды. */
  async getInventory() {
    const data = await this.gql.query('Inventory');
    const inventory = data?.currentUser?.inventory ?? {};
    return {
      inProgress: inventory.dropCampaignsInProgress ?? [],
      claimedBenefits: inventory.gameEventDrops ?? [],
    };
  }

  /** Полные данные кампании: дропы, прогресс, ACL каналов. */
  async getCampaignDetails(login, campaignId) {
    const data = await this.gql.query('DropCampaignDetails', {
      channelLogin: login,
      dropID: campaignId,
    });
    return data?.user?.dropCampaign ?? null;
  }

  /** Живые каналы игры (по умолчанию — только с включёнными Drops). */
  async getGameStreams(slug, { limit = 30, dropsEnabled = true, sort = 'RELEVANCE' } = {}) {
    const data = await this.gql.query('GameDirectory', {
      slug,
      limit,
      options: {
        broadcasterLanguages: [],
        freeformTags: null,
        includeRestricted: ['SUB_ONLY_LIVE'],
        recommendationsContext: { platform: 'web' },
        sort,
        systemFilters: dropsEnabled ? ['DROPS_ENABLED'] : [],
        tags: [],
      },
    });
    return data?.game?.streams?.edges?.map((edge) => edge.node).filter(Boolean) ?? [];
  }

  /** Состояние конкретного канала: онлайн, игра, зрители. */
  async getChannelState(login) {
    const data = await this.gql.query('ChannelStream', { channel: login });
    return data?.user ?? null;
  }

  /** Кампании, доступные на конкретном канале. */
  async getAvailableDrops(channelId) {
    const data = await this.gql.query('AvailableDrops', { channelID: String(channelId) });
    return data?.channel?.viewerDropCampaigns ?? [];
  }

  async getPlaybackToken(login) {
    const data = await this.gql.query('PlaybackAccessToken', { login });
    return data?.streamPlaybackAccessToken ?? null;
  }

  /** Список вариантов качества (master playlist) для канала. */
  async getMasterPlaylist(login) {
    const token = await this.getPlaybackToken(login);
    if (!token?.value || !token?.signature) return null;
    const url =
      `${USHER_URL}/${login}.m3u8?allow_source=true&allow_audio_only=true&` +
      `sig=${encodeURIComponent(token.signature)}&token=${encodeURIComponent(token.value)}`;
    const response = await request(url, { parse: 'text', retries: 2 });
    const text = String(response.data ?? '');
    return { url, text };
  }

  /** Прогресс текущего дропа по данным сервера. */
  async getCurrentDrop(channelId) {
    const data = await this.gql.query('CurrentDrop', { channelID: String(channelId) });
    return data?.currentUser?.dropCurrentSession ?? null;
  }

  async claimDrop(dropInstanceId) {
    const data = await this.gql.query('ClaimDrop', { input: { dropInstanceID: dropInstanceId } });
    const result = data?.claimDropRewards;
    return (
      result?.status === 'ELIGIBLE_FOR_ALL' ||
      result?.status === 'DROP_INSTANCE_ALREADY_CLAIMED'
    );
  }

  /** Проверка доступности операций (для selftest). */
  async probe() {
    const report = [];
    const safe = async (name, fn) => {
      try {
        const value = await fn();
        report.push({ name, ok: true, info: value });
      } catch (error) {
        report.push({ name, ok: false, info: error.message, code: error.code });
      }
    };

    await safe('GameDirectory', async () => {
      const streams = await this.getGameStreams('fortnite', { limit: 3 });
      return `каналов: ${streams.length}`;
    });
    await safe('PlaybackAccessToken', async () => {
      const [stream] = await this.getGameStreams('fortnite', { limit: 1, dropsEnabled: false });
      if (!stream) return 'нет живых каналов для проверки';
      const token = await this.getPlaybackToken(stream.broadcaster.login);
      return token?.value ? 'токен получен' : 'пустой токен';
    });
    await safe('MasterPlaylist', async () => {
      const [stream] = await this.getGameStreams('fortnite', { limit: 1, dropsEnabled: false });
      if (!stream) return 'нет живых каналов для проверки';
      const playlist = await this.getMasterPlaylist(stream.broadcaster.login);
      return playlist?.text?.includes('#EXTM3U') ? 'плейлист получен' : 'плейлист пуст';
    });
    await safe('WatchTick', async () => {
      const [stream] = await this.getGameStreams('fortnite', { limit: 1, dropsEnabled: false });
      if (!stream) return 'нет живых каналов для проверки';
      const playlist = await this.getMasterPlaylist(stream.broadcaster.login);
      const variant = String(playlist?.text ?? '')
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.startsWith('http'))
        .at(-1);
      if (!variant) return 'нет варианта качества';
      const chunkList = await request(variant, { parse: 'text', timeoutMs: 15000 });
      const chunk = String(chunkList.data ?? '')
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.startsWith('http'))
        .at(-1);
      if (!chunk) return 'нет чанков';
      const head = await request(chunk, { method: 'HEAD', parse: 'text', retries: 0, timeoutMs: 10000 });
      return head.status === 200 ? 'чанк доступен (200)' : `чанк вернул ${head.status}`;
    });
    return report;
  }
}

export function emptyGqlOperation(name) {
  return GQL_OPS[name];
}
