// Runs the real Svelte pages against synthetic IPC, using an existing Chromium binary.
// Start tests/categories/vite.config.mjs on port 1426 first. No app database is opened.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
const profile = await mkdtemp(path.join(tmpdir(), 'pomotroid-ui-'));
const chrome = spawn(
  process.env.CHROME_PATH ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe',
  [
    '--headless=new',
    '--remote-debugging-port=1436',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    'about:blank',
  ],
  { stdio: 'ignore', windowsHide: true }
);
let socket;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let id = 0;
const pending = new Map();
const errors = [];
function send(method, params = {}) {
  const request = ++id;
  return new Promise((resolve, reject) => {
    pending.set(request, { resolve, reject });
    socket.send(JSON.stringify({ id: request, method, params }));
  });
}
async function evaluate(expression) {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails)
    throw new Error(r.exceptionDetails.text + ': ' + r.exceptionDetails.exception?.description);
  return r.result.value;
}
async function until(expression) {
  for (let i = 0; i < 100; i++) {
    if (await evaluate(expression)) return;
    await sleep(80);
  }
  throw new Error(
    `Timeout: ${expression}\n${await evaluate('document.body.innerText')}\n${JSON.stringify(errors)}`
  );
}
async function click(selector) {
  await evaluate(
    `(()=>{const e=document.querySelector(${JSON.stringify(selector)});e.focus();e.click()})()`
  );
  await sleep(50);
}
async function size(width, height) {
  await send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });
}
async function load(kind = 'stats', extra = '') {
  errors.length = 0;
  console.log(
    await send('Page.navigate', {
      url: `http://127.0.0.1:1426/__categories_fixture?data&window=${kind}&${extra}`,
    })
  );
  await until(
    `!!window.categoriesFixture && !!document.querySelector(${JSON.stringify(kind === 'stats' ? '#stats-tab-week' : '.nav-item')})`
  );
  await sleep(150);
}
async function noOverflow() {
  assert.equal(
    await evaluate(
      `document.documentElement.scrollWidth<=innerWidth && [...document.querySelectorAll('.content,.data-flow,dialog[open]')].every(e=>e.scrollWidth<=e.clientWidth+1)`
    ),
    true,
    'horizontal overflow'
  );
}
async function screenshot(name) {
  const { data } = await send('Page.captureScreenshot', { format: 'png' });
  await writeFile(path.resolve('.cache', name + '.png'), Buffer.from(data, 'base64'));
}
try {
  let targets;
  for (let i = 0; i < 100; i++) {
    try {
      targets = await (await fetch('http://127.0.0.1:1436/json')).json();
      break;
    } catch {
      await sleep(100);
    }
  }
  assert.ok(targets?.length, 'Chromium started');
  socket = new WebSocket(
    targets.find((t) => t.type === 'page' && t.url === 'about:blank').webSocketDebuggerUrl
  );
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = reject;
  });
  socket.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id) {
      const p = pending.get(m.id);
      pending.delete(m.id);
      if (m.error) p.reject(new Error(JSON.stringify(m.error)));
      else p.resolve(m.result);
    } else if (m.method === 'Runtime.exceptionThrown')
      errors.push(
        m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text
      );
  };
  await send('Page.enable');
  await send('Runtime.enable');
  await mkdir('.cache', { recursive: true });
  for (const theme of ['dark', 'light']) {
    await size(720, 480);
    await load('stats', `theme=${theme}`);
    await click('#stats-tab-week');
    assert.equal(
      await evaluate(
        `window.categoriesFixture.calls.filter(c=>c.command==='stats_distribution').length`
      ),
      0
    );
    await click('[data-distribution-toggle]');
    await until(`document.querySelectorAll('.distribution .row').length===5`);
    assert.equal(
      await evaluate(`document.querySelector('.distribution .percent').textContent`),
      '60%'
    );
    assert.equal(
      await evaluate(`document.querySelector('.distribution .track>span').style.width`),
      '60%'
    );
    await click('.distribution .more');
    await until(`document.querySelectorAll('.distribution .row').length===7`);
    assert.equal(await evaluate(`document.querySelectorAll('.distribution .row').length`), 7);
    await click('.distribution .row');
    await until(`!!document.querySelector('[data-distribution-return]')`);
    assert.equal(
      await evaluate(`document.activeElement.hasAttribute('data-distribution-return')`),
      true
    );
    assert.equal(
      await evaluate(`document.querySelector('#stats-tab-week').getAttribute('aria-selected')`),
      'true'
    );
    await click('[data-distribution-return]');
    await until(`document.querySelectorAll('.distribution .row').length===7`);
    await click('.metric-switch button:last-child');
    assert.equal(await evaluate(`document.querySelector('.distribution .percent').textContent`),'60%');
    await click('.distribution .more');
    assert.equal(await evaluate(`document.querySelectorAll('.distribution .row').length`),5);
    await click('.distribution .more');
    for (const [w, h] of [
      [720, 480],
      [900, 600],
      [1200, 800],
    ]) {
      await size(w, h);
      await sleep(80);
      await noOverflow();
      assert.ok(
        await evaluate(
          `document.querySelector('.bar-chart')?.getBoundingClientRect().height>=190 || document.querySelector('.plot')?.getBoundingClientRect().height>=190 || document.querySelector('.chart-section svg')?.getBoundingClientRect().height>=190`
        )
      );
    }
    await size(720, 480);
    await evaluate(`document.querySelector('.content').scrollTop=document.querySelector('.content').scrollHeight`);
    await screenshot(`data-week-${theme}`);
    await click('#stats-tab-alltime');
    await click('[data-distribution-toggle]');
    await until(`document.querySelectorAll('.distribution .row').length===5`);
    await noOverflow();
    const square = await evaluate(
      `(()=>{const e=document.querySelector('.heatmap .cell');const r=e.getBoundingClientRect();return Math.abs(r.width-r.height)<1})()`
    );
    assert.equal(square, true);
    await click('.export-button');
    await until(
      `!!document.querySelector('dialog[open]') && !!document.querySelector('dialog .actions .primary:not(:disabled)')`
    );
    const scope = await evaluate(
      `window.categoriesFixture.calls.filter(c=>c.command==='report_preview').at(-1).args.scope`
    );
    assert.equal(scope.start, `${new Date().getFullYear()}-01-01`);
    assert.equal(scope.end, `${new Date().getFullYear()}-12-31`);
    await noOverflow();
    await screenshot(`data-report-${theme}`);
    await send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27,
    });
    await send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27,
    });
    await until(`!document.querySelector('dialog[open]')`);
    assert.equal(
      await evaluate(`document.activeElement.classList.contains('export-button')`),
      true
    );
    assert.deepEqual(errors, []);
    await click('#stats-tab-today');
    await until(`!!document.querySelector('[data-hour="8"]')`);
    await evaluate(`document.querySelector('[data-hour="8"]').dispatchEvent(new MouseEvent('click',{bubbles:true}))`);
    await until(`!!document.querySelector('[data-detail-entry="today-hour"]')`);
    await click('[data-detail-entry="today-hour"]');
    await until(`!!document.querySelector('.date-actions [title="导出当前范围…"]')`);
    await click('.date-actions [title="导出当前范围…"]');
    await until(`!!document.querySelector('dialog .actions .primary:not(:disabled)')`);
    assert.equal(await evaluate(`window.categoriesFixture.calls.filter(c=>c.command==='report_preview').at(-1).args.scope.hour`),8);
    assert.equal(await evaluate(`document.querySelector('dialog .fields select').disabled`),true);
    await evaluate(`(()=>{const e=document.querySelectorAll('dialog .fields select')[1];e.value='recent';e.dispatchEvent(new Event('change',{bubbles:true}));})()`);
    await until(`window.categoriesFixture.calls.filter(c=>c.command==='report_preview').at(-1).args.scope.hour===null`);
    assert.ok(await evaluate(`document.querySelector('dialog').textContent.includes('原小时条件已清除')`));
    await load('settings', `theme=${theme}`);
    await evaluate(
      `[...document.querySelectorAll('.nav-item')].find(e=>e.textContent.trim()==='数据').click()`
    );
    await until(`!!document.querySelector('#data-export-entry')`);
    for (const [w, h] of [
      [780, 620],
      [640, 480],
    ]) {
      await size(w, h);
      await noOverflow();
    }
    await screenshot(`data-settings-${theme}`);
    await click('#data-export-entry');
    assert.deepEqual(
      await evaluate(
        `[...document.querySelectorAll('.data-flow input[type=checkbox]')].map(e=>e.checked)`
      ),
      [true, true, false]
    );
    await noOverflow();
    await click('.data-flow .back');
    await click('#data-import-entry');
    await until(`!!document.querySelector('.data-flow table')`);
    await noOverflow();
    await screenshot(`data-import-${theme}`);
    await click('.data-flow .checks input');
    await until(`!document.querySelector('.data-flow table tbody tr:nth-child(2)')`);
    assert.equal(
      await evaluate(
        `window.categoriesFixture.calls.filter(c=>c.command==='data_replan').at(-1).args.options.history`
      ),
      false
    );
    await click('.data-flow .back');
    await click('#data-report-entry');
    await until(`!!document.querySelector('.data-flow .actions .primary:not(:disabled)')`);
    await noOverflow();
    assert.deepEqual(errors, []);
  }
  await load('settings','cancel');
  await evaluate(`[...document.querySelectorAll('.nav-item')].find(e=>e.textContent.trim()==='数据').click()`);
  await click('#data-import-entry');await until(`!!document.querySelector('#data-import-entry')`);
  assert.equal(await evaluate(`!!document.querySelector('[role=alert]')`),false);
  await click('#data-export-entry');await click('.data-flow .actions .primary');
  await until(`!!document.querySelector('.data-flow .actions .primary:not(:disabled)')`);
  assert.equal(await evaluate(`!!document.querySelector('[role=alert]')`),false);
  await load('settings','invalid');
  await evaluate(`[...document.querySelectorAll('.nav-item')].find(e=>e.textContent.trim()==='数据').click()`);
  await click('#data-import-entry');await until(`!!document.querySelector('[role=alert]')`);
  assert.ok(await evaluate(`document.querySelector('[role=alert]').textContent.includes('版本')`));
  await load('stats','distribution-error');await click('#stats-tab-week');await click('[data-distribution-toggle]');
  await until(`!!document.querySelector('.distribution [role=status] button')`);
  assert.equal(await evaluate(`document.querySelector('.distribution').textContent.includes('暂无已完成')`),false);
  console.log(
    'PASS: dark/light settings 780x620,640x480; stats 720x480,900x600,1200x800; distribution lazy loading/top five/show all/60% bars/metric independence/filter/back/focus/error; square heatmap; yearly and hourly export scope/dialog/Escape; hour clearing; import options/replan; quiet cancellation/invalid package; no overflow or runtime errors.'
  );
} finally {
  socket?.close();
  chrome.kill();
  await new Promise((r) => (chrome.exitCode !== null ? r() : chrome.once('exit', r)));
  const resolved = path.resolve(profile),
    prefix = path.resolve(tmpdir()) + path.sep + 'pomotroid-ui-';
  if (resolved.startsWith(prefix))
    await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }).catch(
      () => {}
    );
}
