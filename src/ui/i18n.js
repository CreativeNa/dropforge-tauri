(function () {
  const EN = {
    'Поиск игры': 'Search games',
    'Файл:': 'File:',
    'Раз в 10 минут': 'Every 10 minutes',
    'Найдите игру': 'Find a game',
    'Найти игру': 'Find a game',
    'Дропы': 'Drops',
    'Бейджи': 'Badges',
    'Стрики': 'Streaks',
    'Настройки': 'Settings',
    'Журнал': 'Log',
    'Обновить': 'Refresh',
    'Обновлено': 'Updated',
    'Обновить список': 'Refresh list',
    'Переподключить Twitch': 'Reconnect Twitch',
    'Переподключение Twitch': 'Reconnect Twitch',
    'Выйти': 'Sign out',
    'Запустить автофарм': 'Start autofarm',
    'Очистить очередь': 'Clear queue',
    'Фармить': 'Farm',
    'Смотреть': 'Watch',
    'Остановить': 'Stop',
    'Стоп': 'Stop',
    'Забрать на Twitch': 'Claim on Twitch',
    'Забирать сундуки': 'Claim chests',
    'Проверить': 'Check',
    'Проверить обновления': 'Check for updates',
    'Отмена': 'Cancel',
    'Условия': 'Terms',
    'Пока нечего забирать': 'Nothing to claim yet',
    'Очередь очищена': 'Queue cleared',
    'Очередь пуста — добавьте игру из списка': 'Queue is empty — add a game from the list',
    'Выберите игру и нажмите «Фармить»': 'Pick a game and press “Farm”',
    'Фарм запущен': 'Farming started',
    'Фарм не запущен': 'Farming stopped',
    'Прогресс': 'Progress',
    'Очередь': 'Queue',
    'Сейчас в фарме': 'Now farming',
    'Сейчас в эфире': 'Live now',
    'Нет каналов в эфире': 'No channels live',
    'Никто из подписок не в эфире': 'None of your subscriptions is live',
    'Пока никто из подписок не в эфире. Как только стример начнёт трансляцию, он появится здесь.':
      'None of your subscriptions is live yet. When a streamer goes live, they will appear here.',
    'Все подписки': 'All subscriptions',
    'Серии под контролем': 'Streaks under control',
    'Серии просмотра каналов из подписок': 'Watch streaks for channels you follow',
    'Анти-стрикосбиватель': 'Streak keeper',
    'Автопилот включён': 'Autopilot on',
    'Автопилот выключен': 'Autopilot off',
    'Смотреть без ограничения': 'Watch without limit',
    'Смотреть сейчас': 'Watch now',
    'смотреть сейчас': 'watch now',
    'Каналов в сутки': 'Channels per day',
    'Смотрел сегодня': 'Watched today',
    'Серия продлена сегодня': 'Streak extended today',
    'В эфире': 'Live',
    'в эфире': 'live',
    'с серией': 'with streak',
    'серия': 'streak',
    'нет данных': 'no data',
    'нет': 'none',
    'лучшая серия': 'best streak',
    'лучший прирост': 'best gain',
    'просмотров': 'views',
    'минут': 'minutes',
    'баллов': 'points',
    'продлений': 'extensions',
    'сундуков': 'chests',
    'бонус': 'bonus',
    'Просмотров этого канала ещё не было. Запусти просмотр — здесь появятся время, баллы и результат.':
      'No watch sessions for this channel yet. Start watching — time, points and results will appear here.',
    'Показаны последние': 'Showing last',
    'Автофарм': 'Autofarm',
    'Поведение автофарма и аккаунт': 'Autofarm behaviour and account',
    'Как приложение ведёт себя, когда награды уже капают.': 'How the app behaves while rewards are being earned.',
    'Автоматически забирать награды': 'Claim rewards automatically',
    'Как только дроп добит до 100%': 'As soon as a drop reaches 100%',
    'Отправлять события просмотра (spade)': 'Send watch events (spade)',
    'Через эти события Twitch засчитывает минуты просмотра. Выключайте только если Twitch начал ограничивать запросы — тогда прогресс дропов остановится':
      'Twitch counts watch minutes through these events. Turn off only if Twitch starts rate-limiting — drop progress will stop.',
    'Возобновлять фарм после перезапуска': 'Resume farming after restart',
    'Если приложение закрылось, фарм продолжится сам в этом окне. 0 — не возобновлять':
      'If the app closes, farming resumes in this window. 0 — do not resume.',
    'Сворачивать в трей при закрытии': 'Minimize to tray on close',
    'Крестик прячет окно, фарм продолжается. Выключите, чтобы крестик завершал приложение':
      'The close button hides the window and farming continues. Turn off to make it quit the app.',
    'Обновлять склад кампаний': 'Refresh campaign catalog',
    'Обслуживание': 'Maintenance',
    'Диагностика': 'Diagnostics',
    'Соберёт версии, состояние сервисов, доступность Twitch и журнал. Без логина, токена и путей с именем пользователя':
      'Collects versions, service state, Twitch availability and the log. No login, token or user paths.',
    'Собрать отчёт': 'Collect report',
    'Скопировать журнал': 'Copy log',
    'Открыть файл журнала': 'Open log file',
    'Последние события': 'Recent events',
    'Те же записи, что попадают в отчёт диагностики': 'Same entries as in the diagnostics report',
    'О приложении': 'About',
    'Версия': 'Version',
    'Сборка': 'Build',
    'Язык': 'Language',
    'Обновления': 'Updates',
    'Проверяет релизы на GitHub и ставит новую версию сама': 'Checks GitHub releases and installs new versions automatically',
    'Установлена последняя версия': 'You have the latest version',
    'Доступна версия': 'Version available',
    'Страница проекта на GitHub': 'Project page on GitHub',
    'Смена стримера на лету.': 'Switches streamer on the fly.',
    'Стрим закончился — приложение переключится само.': 'The stream ended — the app will switch automatically.',
    'Игры с дропами': 'Games with drops',
    'Игры': 'Games',
    'Дропы копятся,': 'Drops accumulate,',
    'Выбрать всё доступное': 'Select all available',
    'Сбросить фильтр': 'Clear filter',
    'Ничего не найдено': 'Nothing found',
    'Измените запрос или обновите список — возможно, кампании ещё не начались.':
      'Change the query or refresh the list — campaigns may not have started yet.',
    'Кампаний нет': 'No campaigns',
    'Для этой игры сейчас не проводятся Drops.': 'No Drops are running for this game right now.',
    'Бейджей пока нет': 'No badges yet',
    'Обновите каталог — кампании со значками появятся автоматически.':
      'Refresh the catalog — badge campaigns will appear automatically.',
    'Бесплатные': 'Free',
    'Платные': 'Paid',
    'Платный': 'Paid',
    'получен': 'claimed',
    'получена': 'claimed',
    'получено': 'claimed',
    'готово': 'ready',
    'готово к получению': 'ready to claim',
    'всё получено': 'all claimed',
    'все цепочки получены': 'all chains completed',
    'награда Twitch': 'Twitch reward',
    'награда за подписку на канал': 'channel subscription reward',
    'награда уже на аккаунте': 'reward already on your account',
    'часы считает Twitch — забери награду на twitch.tv/drops/rewards':
      'Twitch counts the hours — claim the reward at twitch.tv/drops/rewards',
    'Проверяйте прогресс на Twitch': 'Track progress on Twitch',
    'открыть категорию': 'open category',
    'Открыть страницу Twitch': 'Open Twitch page',
    'Журнал пуст': 'Log is empty',
    'Здесь появятся события: загрузка кампаний, смена стримера, полученные награды.':
      'Events will appear here: campaign loading, streamer switches, claimed rewards.',
    'Журнал скопирован': 'Log copied',
    'Что делает приложение прямо сейчас': 'What the app is doing right now',
    'Файл настроек повреждён.': 'Settings file is corrupted.',
    'Приложение запустилось со значениями по умолчанию. Копия старого файла:':
      'The app started with default values. Copy of the old file:',
    'Аккаунт': 'Account',
    'аккаунт Twitch': 'Twitch account',
    'Подключите аккаунт Twitch': 'Connect your Twitch account',
    'Привязать аккаунт': 'Link account',
    'Вход только на сайте Twitch.': 'Sign in on the Twitch website only.',
    'Завершите вход в открывшемся окне Twitch': 'Finish signing in in the opened Twitch window',
    'Ждём подтверждения в браузере…': 'Waiting for browser confirmation…',
    'Подтвердите вход на странице Twitch — приложение получит доступ само, пароль вводить в него не нужно.':
      'Confirm sign-in on the Twitch page — the app will get access itself, no password needed here.',
    'Приложение не спрашивает пароль и не видит его.': 'The app never asks for or sees your password.',
    'Вход по токену': 'Sign in with token',
    'Вставьте OAuth-токен': 'Paste an OAuth token',
    'Убрать из списка фарма': 'Remove from farming list',
    'Добавить в список, не запуская': 'Add to list without starting',
    'Поставить на паузу': 'Pause',
    'Начать раньше': 'Start earlier',
    'Начать день': 'Start the day',
    'Идёт день:': 'Day in progress:',
    'Загрузка…': 'Loading…',
    'Недоступен': 'Unavailable',
    'Офлайн': 'Offline',
    'Сегодня': 'Today',
    'сегодня': 'today',
    'сейчас': 'now',
    'прошлого запуска': 'previous run',
    'без срока': 'no deadline',
    'до': 'until',
    'мин': 'min',
    'минут': 'minutes',
    'часов': 'hours',
    'игр': 'games',
    'игра': 'game',
    'каналов': 'channels',
    'подписок': 'subscriptions',
    'подписка': 'subscription',
    'подписки': 'subscriptions',
    'канал': 'channel',
    'Канал': 'Channel',
    'каналов в сутки': 'channels per day',
    'Смотреть Drops': 'Watch Drops',
    'В разработке': 'In development',
    'Локальные': 'Local',
    'Локальные дропсы доступны в Tauri-сборке': 'Local drops are available in the Tauri build',
    'Локальные дропсы каналов': 'Local channel drops',
    'Локальные дропсы пока недоступны — раздел в разработке.':
      'Local drops are not available yet — this section is in development.',
    'Локальных дропсов нет': 'No local drops',
    'Локальных дропсов сейчас нет': 'No local drops right now',
    'Проверьте канал — покажем его локальные дропсы': 'Check a channel — its local drops will appear here',
    'Проверьте канал выше — приложение покажет кампании, которые Twitch выдаёт только на нём.':
      'Check a channel above — the app will show campaigns Twitch grants only there.',
    'Просмотр остановлен': 'Watch stopped',
    'Смотрю': 'Watching',
    'Смотрю стрим': 'Watching stream',
    'Этот эфир уже засчитан': 'This stream is already counted',
    'Награды забираются автоматически.': 'Rewards are claimed automatically.',
    'Пока вы заняты': 'While you are busy',
    'другими делами': 'with other things',
    'Дропы': 'Drops',
    'Смотреть': 'Watch',
    'Фармить': 'Farm',
    'нет': 'none',
    'Не удалось скопировать': 'Copy failed',
    'Копирование недоступно': 'Copy is unavailable',
    'Отчёт ещё не собран': 'Report has not been collected yet',
    'Отчёт открыт, текст скопирован в буфер': 'Report opened and copied to clipboard',
    'Отчёт собран, открыт и скопирован в буфер': 'Report collected, opened and copied to clipboard',
    'Подписки обновлены:': 'Subscriptions updated:',
    'Награда получена:': 'Reward claimed:',
    'Сводка:': 'Summary:',
    'Обновление:': 'Update:',
    'Стрики:': 'Streaks:',
    'Диагностика:': 'Diagnostics:',
    'Переподключение:': 'Reconnect:',
    'Найдено кампаний:': 'Campaigns found:',
    'Загружено кампаний:': 'Campaigns loaded:',
    'Смотрю ninja_live — прогресс +1 мин': 'Watching ninja_live — progress +1 min',
    'Вход выполнен': 'Signed in',
    'Вход отменён': 'Sign-in cancelled',
    'Twitch переподключён': 'Twitch reconnected',
    'Начать': 'Start',
    'Серия': 'Streak',
    'Сейчас недоступен': 'Currently unavailable',
    'Каналов в сутки': 'Channels per day',
    'Остановить просмотр': 'Stop watching',
    'Нет игр с доступными дропами': 'No games with available drops',
    'Проверьте канал': 'Check channel',
    'Бесплатных осталось': 'Free left',
    'Всего': 'Total',
    'Получено': 'Claimed',
    'Готовы к выдаче': 'Ready to claim',
    'Накоплено': 'Accumulated',
    'Дропы': 'Drops',
  };

  const PATTERNS = [
    [/^Раз в (\d+) минут(у|ы)?$/, 'Every $1 minutes'],
    [/^(\d+) игр · нажмите на карточку, чтобы увидеть награды$/, '$1 games · click a card to see rewards'],
    [/^(\d+) игр · нажмите на карточку, чтобы увидеть награды\.$/, '$1 games · click a card to see rewards.'],
    [/^(\d+) дропов$/, '$1 drops'],
    [/^(\d+)\s+игр\s+·\s+(\d+)\s+кампаний\s+·\s+(\d+)\s+дропов$/, '$1 games · $2 campaigns · $3 drops'],
    [/^Значки профиля Twitch · (\d+) из (\d+) получено$/, 'Twitch profile badges · $1 of $2 claimed'],
    [/^(\d+) игр$/, '$1 games'],
    [/^(\d+) игра$/, '$1 game'],
    [/^(\d+) дропов$/, '$1 drops'],
    [/^(\d+) каналов$/, '$1 channels'],
    [/^(\d+) подписок$/, '$1 subscriptions'],
    [/^(\d+) подписка$/, '$1 subscription'],
    [/^(\d+) подписки$/, '$1 subscriptions'],
    [/^(\d+) мин$/, '$1 min'],
    [/^(\d+) мин просмотра$/, '$1 min watched'],
    [/^(\d+) ч$/, '$1 h'],
    [/^(\d+) м$/, '$1 m'],
    [/^(\d+) из (\d+)$/, '$1 of $2'],
    [/^Обновлено (.+)$/, 'Updated $1'],
    [/^осталось (.+)$/, 'left $1'],
    [/^до (.+)$/, 'until $1'],
    [/^старт (.+)$/, 'starts $1'],
    [/^Показаны последние (\d+) из (\d+)$/, 'Showing last $1 of $2'],
    [/^получено (\d+) из (\d+)$/, '$1 of $2 claimed'],
    [/^Ежедневный сундук с цепочкой · получено (\d+) из (\d+)$/, 'Daily chest chain · $1 of $2 claimed'],
    [/^Идёт день: (\d+)\/60 мин · следующий сундук откроется через 24 часа после закрытия$/,
      'Day in progress: $1/60 min · next chest opens 24 hours after closing'],
    [/^Убрать все игры из очереди \((\d+)\)\?$/, 'Remove all games from the queue ($1)?'],
    [/^Удалить (.+)\?$/, 'Delete $1?'],
    [/^Через (\d+) (мин|минуты|минут)$/, 'In $1 min'],
    [/^(\d+) (мин|минуты|минут) назад$/, '$1 min ago'],
    [/^(.*) · отмечен$/, '$1 · pinned'],
    [/^Смотрю (.+)$/, 'Watching $1'],
    [/^Стрики: (.+)$/, 'Streaks: $1'],
    [/^Диагностика: (.+)$/, 'Diagnostics: $1'],
    [/^Обновление: (.+)$/, 'Update: $1'],
    [/^Обновления: (.+)$/, 'Updates: $1'],
    [/^Награда получена: (.+)$/, 'Reward claimed: $1'],
    [/^Подписки обновлены: (.+)$/, 'Subscriptions updated: $1'],
    [/^Сводка: (.+)$/, 'Summary: $1'],
    [/^Не удалось проверить: (.+)$/, 'Check failed: $1'],
    [/^Файл: (.+)$/, 'File: $1'],
    [/^Отчёт сохранён: (.+)$/, 'Report saved: $1'],
    [/^Обновляется раз в (\d+) мин$/, 'Refreshes every $1 min'],
    [/^(\d+) (минута|минуты|минут)$/, '$1 min'],
  ];

  const MONTHS = [
    ['янв', 'Jan'], ['фев', 'Feb'], ['мар', 'Mar'], ['апр', 'Apr'],
    ['мая', 'May'], ['май', 'May'], ['июн', 'Jun'], ['июл', 'Jul'],
    ['авг', 'Aug'], ['сен', 'Sep'], ['окт', 'Oct'], ['ноя', 'Nov'], ['дек', 'Dec'],
  ];

  let lang = 'ru';

  function translateString(value) {
    if (lang !== 'en') return value;
    const key = value.trim();
    if (!key) return value;
    let out = EN[key];
    if (!out) {
      for (const [re, replacement] of PATTERNS) {
        if (re.test(key)) {
          out = key.replace(re, replacement);
          break;
        }
      }
    }
    if (!out) return value;
    for (const [ru, en] of MONTHS) {
      out = out.replace(new RegExp(ru, 'g'), en);
    }
    // единицы времени/количества внутри строки
    out = out
      .replace(/(\d+)\s*ч(?=\s|$|[.,])/g, '$1 h')
      .replace(/(\d+)\s*мин(?=\s|$|[.,])/g, '$1 min')
      .replace(/(\d+)\s*м(?=\s|$|[.,])/g, '$1 m')
      .replace(/(\d+)\s*игр(?=\s|$|[.,])/g, '$1 games')
      .replace(/(\d+)\s*дропов(?=\s|$|[.,])/g, '$1 drops');
    return value.replace(key, out);
  }

  function localizeAttributes(el) {
    for (const name of ['placeholder', 'title', 'aria-label']) {
      const value = el.getAttribute?.(name);
      if (value && /[А-Яа-я]/.test(value)) {
        const translated = translateString(value);
        if (translated !== value) el.setAttribute(name, translated);
      }
    }
  }

  function localize(root) {
    if (lang !== 'en' || !root) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    for (const node of nodes) {
      const value = node.nodeValue;
      if (!value || !/[А-Яа-я]/.test(value)) continue;
      const translated = translateString(value);
      if (translated !== value) node.nodeValue = translated;
    }
    if (root.querySelectorAll) {
      for (const el of root.querySelectorAll('[placeholder],[title],[aria-label]')) {
        localizeAttributes(el);
      }
    }
  }

  let scheduled = false;
  function schedule() {
    if (lang !== 'en' || scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      localize(document.body);
    });
  }

  window.dfTranslate = (value) => translateString(String(value ?? ''));
  window.dfSetLanguage = (next) => {
    lang = next === 'en' ? 'en' : 'ru';
    document.documentElement.lang = lang;
    if (lang === 'en') localize(document.body);
  };

  if (document.body) {
    new MutationObserver(schedule).observe(document.body, {
      childList: true,
      subtree: true,
      characterData: true,
    });
  } else {
    window.addEventListener('DOMContentLoaded', () => {
      new MutationObserver(schedule).observe(document.body, {
        childList: true,
        subtree: true,
        characterData: true,
      });
    });
  }
})();
