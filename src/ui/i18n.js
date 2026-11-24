(function () {
  const PACK = window.DF_EN || {};
  const EN = PACK.strings || {};
  const PATTERNS = PACK.patterns || [];
  const MONTHS = PACK.months || [];
  const LOG_PATTERNS = PACK.logs || [];










  function translateLog(value) {
    if (lang !== 'en') return value;
    const text = String(value ?? '');
    for (const [re, replacement] of LOG_PATTERNS) {
      if (re.test(text)) {
        return text
          .replace(re, replacement)
          .replace(/· \+(\d+) баллов/g, '· +$1 points')
          .replace(/(\d+) мин/g, '$1 min')
          .replace(/баланс/g, 'balance')
          .replace(/награда/g, 'reward');
      }
    }
    return text;
  }

  function normalize(value) {
    return String(value ?? '').replace(/\s+/g, ' ').trim();
  }

  const EN_NORM = new Map(Object.entries(EN).map(([key, value]) => [normalize(key), value]));

  let lang = 'ru';

  function translateString(value) {
    if (lang !== 'en') return value;
    const key = normalize(value);
    if (!key) return value;
    let out = EN_NORM.get(key);
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
    const original = String(value);
    return original.replace(original.trim(), out);
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
    if (root.querySelectorAll) {
      for (const el of root.querySelectorAll('p,h1,h2,h3,li')) {
        if (!el.childElementCount) continue;
        const text = normalize(el.textContent);
        let translated = EN_NORM.get(text);
        if (!translated && /^Найдите .*Войти.*Twitch\.$/.test(text)) {
          translated = 'Press “Sign in” — a browser opens where you are (or will be) signed in to Twitch.';
        }
        if (!translated && /^Любая игра с Drops\..*$/.test(text)) {
          translated = 'Any game with Drops. Campaigns and progress are pulled from your account.';
        }
        if (translated && el.textContent !== translated) el.textContent = translated;
      }
    }
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
  window.dfTranslateLog = translateLog;
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
