/**
 * Константы Twitch: клиенты, GQL-операции, топики PubSub.
 *
 * Хэши persisted-запросов — это публичные идентификаторы операций Twitch GraphQL.
 * Twitch периодически их обновляет: если операция перестаёт отвечать
 * `PersistedQueryNotFound`, запустите `npm run selftest`, чтобы увидеть,
 * какие операции устарели.
 */

export const PUBSUB_URL = 'wss://pubsub-edge.twitch.tv/v1';

export const GQL_URL = 'https://gql.twitch.tv/gql';
export const USHER_URL = 'https://usher.ttvnw.net/api/channel/hls';

/**
 * Профили клиентов. Client-Id должен совпадать с тем, на который выдан токен,
 * иначе Twitch вернёт 401 на GraphQL.
 */
export const CLIENTS = {
  web: {
    id: 'kimne78kx3ncx6brgo4mv6wki5h1ko',
    origin: 'https://www.twitch.tv',
    referer: 'https://www.twitch.tv/',
    userAgent:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
      '(KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
    label: 'Веб-клиент',
  },
  android: {
    id: 'kd1unb4b3q4t58fwlpcbzcbnm76a8fp',
    origin: 'https://www.twitch.tv',
    referer: 'https://www.twitch.tv/',
    userAgent:
      'Dalvik/2.1.0 (Linux; U; Android 15; SM-S911B Build/TP1A.220624.014) ' +
      'tv.twitch.android.app/31.8.0/3108006',
    label: 'Android-приложение',
  },
  smartbox: {
    id: 'ue6666qo983tsx6so1t0vnawi233wa',
    origin: 'https://android.tv.twitch.tv',
    referer: 'https://android.tv.twitch.tv/',
    userAgent:
      'Mozilla/5.0 (Linux; Android 7.1; Smart Box C1) AppleWebKit/537.36 ' +
      '(KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
    label: 'Smart TV',
  },
  /**
   * Клиент для входа по коду устройства: Twitch отдаёт его в OAuth Device Flow
   * и сразу подставляет код в ссылку активации — пользователю остаётся
   * подтвердить вход в браузере.
   */
  mobileWeb: {
    id: 'r8s4dac0uhzifbpu9sjdiwzctle17ff',
    origin: 'https://m.twitch.tv',
    referer: 'https://m.twitch.tv/',
    userAgent:
      'Mozilla/5.0 (Linux; Android 16; SM-S911B) AppleWebKit/537.36 ' +
      '(KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36',
    label: 'Мобильный веб-клиент',
  },
};

/**
 * Клиент, на который Twitch выдаёт токен при входе по коду устройства.
 *
 * Важно: gql.twitch.tv принимает Client-Id только от «своих» клиентов
 * (веб, Android-приложение, Smart TV). Мобильный веб-клиент r8s4… на GraphQL
 * отвечает «Client-ID header is invalid», поэтому для входа используем
 * Smart TV — он поддерживает и Device Flow, и GraphQL.
 */
export const DEVICE_CLIENT = 'smartbox';

export const CLIENT_BY_ID = Object.fromEntries(
  Object.entries(CLIENTS).map(([key, value]) => [value.id, { key, ...value }]),
);

/** Операции GraphQL: имя -> { operationName, hash }. */
export const GQL_OPS = {
  ViewerDropsDashboard: {
    operationName: 'ViewerDropsDashboard',
    hash: 'c16bb890cc8ce7647a96ee69cd313d423a378a3dedadf630a1017cde18975feb',
    defaults: { fetchRewardCampaigns: false },
  },
  DropCampaignDetails: {
    operationName: 'DropCampaignDetails',
    hash: '039277bf98f3130929262cc7c6efd9c141ca3749cb6dca442fc8ead9a53f77c1',
    defaults: { channelLogin: '', dropID: '' },
  },
  Inventory: {
    operationName: 'Inventory',
    hash: '8337eb8541b314040b0edde0c09c5c7a2783ba1960aa9edfbf3bac16d0fec404',
    defaults: { fetchRewardCampaigns: false },
  },
  AvailableDrops: {
    operationName: 'DropsHighlightService_AvailableDrops',
    hash: '782dad0f032942260171d2d80a654f88bdd0c5a9dddc392e9bc92218a0f42d20',
    defaults: { channelID: '' },
  },
  PlaybackAccessToken: {
    operationName: 'PlaybackAccessToken',
    hash: 'ed230aa1e33e07eebb8928504583da78a5173989fadfb1ac94be06a04f3cdbe9',
    defaults: {
      isLive: true,
      isVod: false,
      login: '',
      platform: 'web',
      playerType: 'site',
      vodID: '',
    },
  },
  GameDirectory: {
    operationName: 'DirectoryPage_Game',
    hash: '86bcceb4e8b1a51256ff8eed8bd8aae4acacf80d737efe904f84f3aeadf8cafd',
    defaults: {
      limit: 30,
      slug: '',
      imageWidth: 50,
      includeCostreaming: false,
      options: {
        broadcasterLanguages: [],
        freeformTags: null,
        includeRestricted: ['SUB_ONLY_LIVE'],
        recommendationsContext: { platform: 'web' },
        sort: 'RELEVANCE',
        systemFilters: ['DROPS_ENABLED'],
        tags: [],
      },
      sortTypeIsRecency: false,
    },
  },
  CurrentDrop: {
    operationName: 'DropCurrentSessionContext',
    hash: '4d06b702d25d652afb9ef835d2a550031f1cf762b193523a92166f40ea3d142b',
    defaults: { channelID: '', channelLogin: '' },
  },
  ClaimDrop: {
    operationName: 'DropsPage_ClaimDropRewards',
    hash: 'a455deea71bdc9015b78eb49f4acfbce8baa7ccbedd28e549bb025bd0f751930',
    defaults: { input: { dropInstanceID: '' } },
  },
  ChannelStream: {
    operationName: 'VideoPlayerStreamInfoOverlayChannel',
    hash: '198492e0857f6aedead9665c81c5a06d67b25b58034649687124083ff288597d',
    defaults: { channel: '' },
  },
};

/** Топики PubSub: прогресс и выдача дропов приходят сюда в реальном времени. */
export const TOPIC = {
  userDrops: (userId) => `user-drop-events.${userId}`,
  channelStreamState: (channelId) => `video-playback-by-id.${channelId}`,
};

/** Сколько секунд «смотрим» между отправками пакетов плейлиста. */
export const WATCH_INTERVAL_SEC = 60;
/** Пауза между попытками переподключения PubSub. */
export const PUBSUB_PING_MS = 4 * 60 * 1000;

/**
 * Каталог кампаний сырым запросом.
 *
 * Persisted-вариант ViewerDropsDashboard (хэш выше) возвращает пустой список,
 * хотя поля в схеме живы — этот же набор полей обычным запросом работает.
 */
export const RAW_DROPS_DASHBOARD = `query ViewerDropsDashboard {
  currentUser {
    id
    dropCampaigns {
      id
      name
      status
      startAt
      endAt
      accountLinkURL
      imageURL
      game { id name slug displayName boxArtURL }
      allow { isEnabled channels { id name } }
      timeBasedDrops {
        id
        name
        startAt
        endAt
        requiredMinutesWatched
        preconditionDrops { id }
        benefitEdges { benefit { id name distributionType imageAssetURL } }
        self { currentMinutesWatched dropInstanceID isClaimed }
      }
    }
  }
}`;
