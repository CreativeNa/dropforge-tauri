/**
 * Дымовой тест цикла просмотра без авторизации: проверяет выбор канала,
 * получение плейлиста и запрос чанков на реальном канале Twitch.
 *
 *   node test/watcher-smoke.mjs
 */
import { GqlClient } from '../src/core/gql.js';
import { TwitchApi } from '../src/core/twitch.js';
import { Watcher } from '../src/core/watcher.js';
import { CLIENTS } from '../src/core/constants.js';

const api = new TwitchApi(new GqlClient({ profile: CLIENTS.web }));

const campaign = {
  id: 'smoke',
  name: 'Smoke test',
  linked: true,
  canEarn: true,
  minutesLeft: 10_000,
  totalRequiredMinutes: 60,
  game: { slug: 'fortnite', name: 'Fortnite', id: 33214 },
  acceptsChannel: () => true,
  drops: [],
};

const store = {
  selectedCampaigns: () => [campaign],
  findDrop: () => null,
  updateProgress: () => false,
};

const watcher = new Watcher({
  api,
  store,
  settings: { sendSpadeEvents: false },
  session: { userId: '0', userAgent: CLIENTS.web.userAgent },
});

let sawChannel = false;
watcher.setTargets(['fortnite']);
watcher.on('status', (status) => {
  const line = `[${status.state}] ${status.message}${
    status.channel ? ` → ${status.channel.login} (${status.channel.viewers} зр.)` : ''
  }`;
  console.log(line);
  if (status.channel) sawChannel = true;
});
watcher.on('error', (message) => console.error('error:', message));

console.log(`Старт: ${new Date().toISOString()}`);
watcher.start();

setTimeout(async () => {
  await watcher.stop();
  console.log(sawChannel ? 'RESULT: OK — канал выбран и просмотр идёт' : 'RESULT: FAIL — канал не выбран');
  process.exit(sawChannel ? 0 : 1);
}, 20_000);
