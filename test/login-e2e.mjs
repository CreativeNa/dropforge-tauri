/**
 * E2E-проверка входа через браузер: клик по кнопке в реальном окне приложения
 * через DevTools-протокол. Требует запущенного приложения с
 * --remote-debugging-port=9222.
 */

const BASE = 'http://127.0.0.1:9222';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function getPage() {
  for (let i = 0; i < 30; i += 1) {
    try {
      const res = await fetch(`${BASE}/json/list`);
      const list = await res.json();
      const page = list.find((t) => t.type === 'page' && t.url.includes('index.html'));
      if (page) return page;
    } catch {
      /* приложение ещё поднимается */
    }
    await sleep(500);
  }
  throw new Error('Не дождался страницы приложения');
}

const page = await getPage();
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener('open', resolve, { once: true });
  ws.addEventListener('error', reject, { once: true });
});

let counter = 0;
const pending = new Map();
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
});

function evaluate(expression) {
  const id = ++counter;
  ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true, awaitPromise: true } }));
  return new Promise((resolve) => pending.set(id, resolve)).then((msg) => msg.result?.result?.value);
}

const initial = await evaluate(`document.querySelector('[data-action="login-device"]')?.textContent.trim()`);
console.log('кнопка входа:', JSON.stringify(initial));

await evaluate(`document.querySelector('[data-action="login-device"]').click(); 'ok'`);
console.log('нажали «Войти через Twitch»');

let code = null;
for (let i = 0; i < 20; i += 1) {
  await sleep(500);
  code = await evaluate(`document.querySelector('.device-code .code')?.textContent.trim() ?? null`);
  if (code) break;
}

console.log('код в интерфейсе:', code);
const status = await evaluate(`document.querySelector('.device-code .device-status')?.textContent.trim() ?? null`);
console.log('статус:', status);

await evaluate(`document.querySelector('[data-action="cancel-device"]')?.click(); 'ok'`);
await sleep(1200);
const afterCancel = await evaluate(`Boolean(document.querySelector('.device-code'))`);
const errorBox = await evaluate(`document.querySelector('.error-box')?.textContent.trim() ?? null`);
console.log('после отмены блок кода:', afterCancel, '| ошибка:', errorBox);

ws.close();
if (!code) {
  console.error('RESULT: FAIL — код не появился');
  process.exit(1);
}
if (afterCancel) {
  console.error('RESULT: FAIL — карточка кода не закрылась');
  process.exit(1);
}
console.log('RESULT: OK — вход запускается из интерфейса, код показывается, отмена работает');
