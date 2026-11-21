#!/usr/bin/env node
/**
 * Консольный интерфейс DropForge — тот же движок, что и в GUI.
 *
 *   node cli.js selftest          проверить доступность запросов Twitch
 *   node cli.js login             войти по коду устройства
 *   node cli.js token <OAuth>     войти по готовому токену
 *   node cli.js status            показать аккаунт и склад кампаний
 *   node cli.js games             список игр с активными дропами
 *   node cli.js farm --game <slug> [--minutes 30]
 */
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Engine } from './src/core/engine.js';
import { GqlClient } from './src/core/gql.js';
import { TwitchApi } from './src/core/twitch.js';
import { CLIENTS } from './src/core/constants.js';
import { formatDuration } from './src/core/util.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const dataDir = process.env.DROPFORGE_DATA || path.join(here, 'data');

const argv = process.argv.slice(2);
const command = argv[0] ?? 'help';
const flags = Object.fromEntries(
  argv
    .slice(1)
    .join(' ')
    .split('--')
    .filter(Boolean)
    .map((chunk) => {
      const [key, ...rest] = chunk.trim().split(/\s+/);
      return [key, rest.join(' ') || true];
    }),
);

const c = {
  dim: (s) => `\x1b[2m${s}\x1b[0m`,
  bold: (s) => `\x1b[1m${s}\x1b[0m`,
  green: (s) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s) => `\x1b[33m${s}\x1b[0m`,
  red: (s) => `\x1b[31m${s}\x1b[0m`,
  cyan: (s) => `\x1b[36m${s}\x1b[0m`,
};

const stamp = () => new Date().toLocaleTimeString('ru-RU');

async function main() {
  const engine = new Engine({ dataDir });
  engine.on('log', (entry) => {
    const color = entry.level === 'error' ? c.red : entry.level === 'success' ? c.green : entry.level === 'warn' ? c.yellow : c.dim;
    console.log(`${c.dim(stamp())} ${color(entry.message)}`);
  });

  switch (command) {
    case 'selftest': {
      console.log(c.bold('Проверка доступности Twitch API (без входа)'));
      const gql = new GqlClient({ profile: CLIENTS.web });
      const api = new TwitchApi(gql);
      const report = await api.probe();
      for (const item of report) {
        console.log(`${item.ok ? c.green(' OK ') : c.red('FAIL')} ${item.name}: ${item.info}`);
      }
      process.exitCode = report.every((item) => item.ok) ? 0 : 1;
      return;
    }

    case 'login': {
      console.log(c.bold('Вход через код устройства'));
      await engine.init();
      await engine.loginWithDeviceCode(({ userCode, uri }) => {
        console.log(`\nКод: ${c.bold(c.cyan(userCode))}`);
        console.log(`Откройте: ${c.cyan(uri)}\n`);
      });
      console.log(c.green(`Готово. Аккаунт: ${engine.user?.login}`));
      return;
    }

    case 'token': {
      const token = argv[1];
      if (!token) throw new Error('Укажите токен: node cli.js token <OAuth>');
      await engine.init();
      await engine.loginWithToken(token);
      console.log(c.green(`Готово. Аккаунт: ${engine.user?.login}`));
      return;
    }

    case 'status':
    case 'games':
    case 'drops': {
      await engine.init();
      if (!engine.loggedIn) throw new Error('Нужен вход: node cli.js login');
      engine.on('refresh', (event) => {
        if (event.message) process.stdout.write(`\r${c.dim(event.message.padEnd(60))}`);
      });
      await engine.refresh();
      process.stdout.write('\r' + ' '.repeat(62) + '\r');
      const snapshot = engine.snapshot();
      printSummary(snapshot);
      if (command === 'drops') printCampaigns(snapshot, flags.game);
      else printGames(snapshot);
      engine.pubsub.stop();
      clearTimeout(engine.refreshTimer);
      process.exit(0);
    }

    case 'farm': {
      await engine.init();
      if (!engine.loggedIn) throw new Error('Нужен вход: node cli.js login');
      await engine.refresh();
      const slugs = flags.game ? [String(flags.game)] : engine.snapshot().games.filter((g) => g.canEarn).map((g) => g.slug);
      if (!slugs.length) throw new Error('Нет игр с доступными дропами');
      await engine.selectOnly(slugs);
      console.log(c.bold(`Фармлю: ${slugs.join(', ')}`));
      await engine.startFarming();
      const limit = Number(flags.minutes) > 0 ? Number(flags.minutes) * 60000 : 0;
      if (limit) setTimeout(() => engine.stopFarming().then(() => process.exit(0)), limit);
      setInterval(() => {
        const { status } = engine.snapshot();
        process.stdout.write(
          `\r${c.dim(stamp())} ${status.channel ? c.cyan(status.channel.login) : '—'} ${status.message.slice(0, 60).padEnd(60)}`,
        );
      }, 1000);
      return;
    }

    default:
      console.log(`
${c.bold('DropForge CLI')} — автофарм Twitch Drops без интерфейса

  ${c.cyan('node cli.js selftest')}                 проверить связь с Twitch
  ${c.cyan('node cli.js login')}                    вход по коду устройства
  ${c.cyan('node cli.js token <OAuth>')}            вход по готовому токену
  ${c.cyan('node cli.js status')}                   аккаунт и статистика
  ${c.cyan('node cli.js games')}                    игры с доступными дропами
  ${c.cyan('node cli.js games --game <slug>')}      подробности по игре
  ${c.cyan('node cli.js farm --game <slug>')}       запустить фарм
`);
  }
}

function printSummary(snapshot) {
  const { auth, stats } = snapshot;
  console.log(c.bold(`\nАккаунт: ${auth.user?.login ?? '—'}`));
  console.log(
    `Кампаний: ${stats.campaigns} · дропов: ${stats.drops} · получено: ${stats.claimed}` +
      ` · накоплено: ${formatDuration(stats.minutesEarned)}`,
  );
}

function printGames(snapshot) {
  const games = [...snapshot.games].sort((a, b) => Number(b.canEarn) - Number(a.canEarn) || a.name.localeCompare(b.name));
  console.log(c.bold('\nИгры с дропами:\n'));
  for (const game of games) {
    const badge = game.canEarn ? c.green('●') : c.dim('○');
    const bar = progressBar(game.totalDrops ? game.claimedDrops / game.totalDrops : 0, 12);
    const ends = game.endsAt ? new Date(game.endsAt).toLocaleDateString('ru-RU') : '—';
    console.log(
      `${badge} ${game.name.padEnd(34).slice(0, 34)} ${bar} ${String(game.claimedDrops).padStart(2)}/${game.totalDrops}  до ${ends}  ${c.dim(game.slug)}`,
    );
  }
}

function printCampaigns(snapshot, slugFilter) {
  const campaigns = snapshot.campaigns.filter(
    (campaign) => !slugFilter || campaign.game.slug === String(slugFilter),
  );
  if (!campaigns.length) {
    console.log(c.yellow('\nНет кампаний по этому фильтру'));
    return;
  }
  for (const campaign of campaigns) {
    console.log(
      `\n${c.bold(campaign.game.name)} — ${campaign.name} ${campaign.expired ? c.red('[завершена]') : campaign.active ? c.green('[активна]') : ''}`,
    );
    for (const drop of campaign.drops) {
      const mark = drop.isClaimed ? c.green('✓') : drop.canClaim ? c.yellow('!') : ' ';
      console.log(
        `  ${mark} ${drop.name.padEnd(38).slice(0, 38)} ${progressBar(drop.progress, 16)} ` +
          `${drop.isClaimed ? 'получено' : `${drop.currentMinutes}/${drop.requiredMinutes} мин`}`,
      );
    }
  }
}

function progressBar(ratio, width) {
  const filled = Math.round(Math.max(0, Math.min(1, ratio)) * width);
  return `${c.green('█'.repeat(filled))}${c.dim('░'.repeat(width - filled))}`;
}

main().catch((error) => {
  console.error(c.red(`Ошибка: ${error.message}`));
  process.exitCode = 1;
});
