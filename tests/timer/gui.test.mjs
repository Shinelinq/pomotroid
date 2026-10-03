// Main timer layout regression: real Svelte page, synthetic IPC, no real database.
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
    `!!window.categoriesFixture && !!document.querySelector(${JSON.stringify(kind === 'main' ? '.timer-outer' : kind === 'stats' ? '#stats-tab-week' : '.nav-item')})`
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
  await size(345, 481);
  await load('main');
  // The standalone fixture host needs the same full-height root as SvelteKit.
  await evaluate(`document.querySelector('#app').style.height='100%'`);
  await until(`!!document.querySelector('.timer-outer')`);
  await evaluate(`document.fonts.ready`);
  await sleep(500);
  await evaluate(`(async()=>{
    window.timerStore = (await import('/src/lib/stores/timer.ts')).timerState;
    window.planStore = (await import('/src/lib/plans/state.ts')).plans;
    window.categoryStore = (await import('/src/lib/categories/state.ts')).categories;
    window.originalDial = document.querySelector('.dial');
    window.layoutWrites = 0;
    new MutationObserver(changes => window.layoutWrites += changes.length)
      .observe(document.querySelector('.timer-outer'), { attributes:true, attributeFilter:['style'] });
  })()`);
  const patch = async (values) => {
    await evaluate(`window.timerStore.update(s => ({...s, ...${JSON.stringify(values)}}))`);
    await sleep(80);
  };
  const geometry = () =>
    evaluate(`(()=>{
    const r = selector => {
      const e = document.querySelector(selector), b = e.getBoundingClientRect(), c = getComputedStyle(e);
      return {x:b.x,y:b.y,w:b.width,h:b.height,cx:b.x+b.width/2,cy:b.y+b.height/2,
        font:parseFloat(c.fontSize), line:parseFloat(c.lineHeight)};
    };
    return {viewport:[innerWidth,innerHeight], stage:r('.timer-stage'),group:r('.timer-outer'),
      dial:r('.dial'),time:r('.time'),play:r('.controls-wrapper .play-pause'),
      previous:r('.controls-wrapper .btn-side'),next:r('.controls-wrapper [aria-label="Skip round"]'),
      reset:r('.btn-text'),rounds:r('.rounds'),sound:r('.volume-wrapper'),
      status:r('.round-status'),label:r('.round-label'),
      compact:document.querySelector('.timer-outer').classList.contains('compact'),
      scale:parseFloat(document.querySelector('.timer-outer').style.getPropertyValue('--main-timer-scale'))};
  })()`);
  const near = (a, b, label) => assert.ok(Math.abs(a - b) < 1, `${label}: ${a} != ${b}`);
  const verify = (g) => {
    near(g.stage.y + g.stage.h, g.viewport[1], 'stage fills real remaining viewport');
    near(g.dial.w, g.dial.h, 'circular dial');
    near(g.group.cy, g.stage.cy, 'complete group vertically centered');
    near(g.dial.cx, g.stage.cx, 'horizontal center');
    assert.ok(
      g.group.y >= g.stage.y + 15 && g.group.y + g.group.h <= g.stage.y + g.stage.h - 15,
      'full height fits'
    );
    assert.ok(g.dial.w <= (g.stage.w - 32) * 0.86 + 1, 'side whitespace');
    assert.ok(g.dial.w <= 314, 'dial cap');
    near(g.time.cx, g.dial.cx, 'time x');
    near(g.time.cy, g.dial.cy, 'time y');
    if (!g.compact) {
      near(g.time.font, (g.dial.w * 48) / 224, 'time scales with dial');
      near(g.play.cx, g.dial.cx, 'play centered');
      near(g.reset.cx, g.play.cx, 'reset centered');
      near(g.previous.cx, g.rounds.cx, 'first column');
      near(g.next.cx, g.sound.cx, 'third column');
      near(g.play.cx - g.previous.cx, g.next.cx - g.play.cx, 'symmetric columns');
      near(g.play.cx - g.previous.cx, 60 * g.scale, 'scaled columns');
      assert.ok(g.play.w >= 44 && g.play.w <= 64.1, 'usable play target');
      near(
        g.group.h,
        g.dial.h +
          Math.min(10, Math.max(6, 8 * g.scale)) +
          g.label.h +
          Math.min(8, Math.max(4, 6 * g.scale)) +
          g.status.h +
          g.play.h +
          2 * Math.min(16, Math.max(8, 12 * g.scale)) +
          g.reset.h,
        'actual height matches budget'
      );
    }
  };
  await patch({
    is_running: false,
    is_paused: false,
    has_started: false,
    total_secs: 1500,
    elapsed_secs: 0,
  });
  const initialCalls = await evaluate('window.categoriesFixture.calls.length');
  const report = [];
  for (const [w, h] of [
    [345, 481],
    [517, 701],
    [760, 480],
    [345, 760],
    [900, 900],
    [240, 240],
    [360, 478],
  ]) {
    await size(w, h);
    await sleep(200);
    const g = await geometry();
    verify(g);
    report.push(g);
    if ((w === 345 && h === 481) || w === 517) await screenshot(`timer-${w}x${h}`);
    assert.equal(
      await evaluate('window.originalDial===document.querySelector(".dial")'),
      true,
      'Timer not remounted'
    );
  }
  assert.equal(
    await evaluate('window.categoriesFixture.calls.length'),
    initialCalls,
    'resize sends no IPC'
  );
  await size(345, 481);
  await sleep(120);
  const statePosition = (await geometry()).dial.cy;
  for (const [type, running, started] of [
    ['work', false, false],
    ['work', true, true],
    ['work', false, true],
    ['short-break', true, true],
    ['long-break', true, true],
  ]) {
    await patch({ round_type: type, is_running: running, has_started: started });
    verify(await geometry());
    near((await geometry()).dial.cy, statePosition, 'status changes keep the time in place');
  }
  const stable = await geometry();
  for (const seconds of [60, 1500, 5400, 7200, 0]) {
    await patch({ total_secs: seconds, elapsed_secs: 0 });
    const g = await geometry();
    verify(g);
    near(g.time.font, stable.time.font, 'standard and three-digit minutes keep font');
    assert.ok(g.time.w < g.dial.w * 0.8, 'time within safe area');
  }
  await patch({ total_secs: 6000000, elapsed_secs: 0 });
  const longTime = await geometry();
  assert.ok(longTime.time.font < stable.time.font && longTime.time.w < longTime.dial.w * 0.8);
  await patch({ total_secs: 1500, elapsed_secs: 0 });
  const writes = await evaluate('window.layoutWrites');
  for (let i = 1; i <= 5; i++) await patch({ elapsed_secs: i });
  assert.equal(await evaluate('window.layoutWrites'), writes, 'ticks do not recalculate layout');
  await evaluate(
    `window.planStore.update(p=>({...p,book:{...p.book,plans:p.book.plans.map(x=>({...x,name:'非常长的专注方案名称用于布局检查',initial_name:false}))}}))`
  );
  await evaluate(
    `window.categoryStore.update(c=>({...c,data:{...c.data,items:c.data.items.map(x=>({...x,name:'非常长的分类名称用于布局检查'}))}}))`
  );
  for (const arranged of [
    { stop_after_round: true },
    { category_pending: true, next_category_id: 2 },
    { category_notice_id: 3 },
  ]) {
    await patch(arranged);
    verify(await geometry());
    assert.equal(
      await evaluate('document.querySelectorAll(".arranged").length'),
      1,
      'one compact notice row'
    );
  }
  await evaluate(`window.planStore.update(p=>({...p,pending_id:'plan-2'}))`);
  await sleep(80);
  verify(await geometry());
  const beforeMenu = await geometry();
  await click('.arranged .trigger');
  await sleep(100);
  near((await geometry()).dial.w, beforeMenu.dial.w, 'popover is outside height budget');
  assert.ok(await evaluate('!!document.querySelector(":popover-open")'));
  await size(517, 701);
  await sleep(200);
  verify(await geometry());
  assert.ok(
    await evaluate(`(()=>{
    const panel=document.querySelector(':popover-open').getBoundingClientRect();
    const trigger=document.querySelector('.arranged .trigger').getBoundingClientRect();
    return panel.x>=7 && panel.right<=innerWidth-7 && panel.y>=7 && panel.bottom<=innerHeight-7
      && (Math.abs(panel.y-trigger.bottom-4)<1 || Math.abs(trigger.y-panel.bottom-4)<1);
  })()`),
    'open menu follows resized trigger'
  );
  await evaluate(`document.querySelector(':popover-open').hidePopover()`);
  const beforeTooltip = (await geometry()).dial.w;
  await evaluate(
    `document.querySelector('.controls-wrapper .tooltip-wrapper').dispatchEvent(new PointerEvent('pointerenter'))`
  );
  await until(`!!document.querySelector('.timer [role=tooltip].positioned')`);
  near((await geometry()).dial.w, beforeTooltip, 'tooltip outside height budget');
  await size(345, 481);
  await sleep(150);
  assert.ok(
    await evaluate(`(()=>{
    const tip=document.querySelector('.timer [role=tooltip]').getBoundingClientRect();
    const button=document.querySelector('.controls-wrapper .btn-side').getBoundingClientRect();
    return tip.x>=7 && tip.right<=innerWidth-7 && Math.abs(tip.bottom+8-button.y)<1;
  })()`),
    'visible tooltip repositions after resize'
  );
  await evaluate(
    `document.querySelector('.controls-wrapper .tooltip-wrapper').dispatchEvent(new PointerEvent('pointerleave'))`
  );
  await size(517, 701);
  await sleep(150);
  await patch({ work_round_number: 1234567, work_rounds_total: 9999999 });
  assert.equal(
    await evaluate(`document.querySelector('.rounds').textContent.split(' ').join('').trim()`),
    '1234567|9999999'
  );
  assert.ok(
    await evaluate(
      `(()=>{const a=document.querySelector('.rounds').getBoundingClientRect(),b=document.querySelector('.btn-text').getBoundingClientRect();return a.right<b.left})()`
    ),
    'long round label does not overlap reset'
  );
  for (const factor of [1, 1.25, 1.5]) {
    await send('Emulation.setDeviceMetricsOverride', {
      width: 345,
      height: 481,
      deviceScaleFactor: factor,
      mobile: false,
    });
    await sleep(100);
    verify(await geometry());
    assert.ok(
      await evaluate(
        `(()=>{const a=document.querySelector('.rounds').getBoundingClientRect(),b=document.querySelector('.btn-text').getBoundingClientRect();return a.right<b.left && a.height<=b.height+1})()`
      ),
      'long counter fits at small sizes'
    );
  }
  await size(240, 240);
  await sleep(100);
  await patch({ stop_after_round: false, category_pending: false, category_notice_id: null });
  await evaluate(`window.planStore.update(p=>({...p,pending_id:null}))`);
  await size(345, 481);
  await sleep(150);
  verify(await geometry());
  await evaluate(`document.querySelector('.timer-stage').style.display='none'`);
  await sleep(100);
  await evaluate(`document.querySelector('.timer-stage').style.display=''`);
  await sleep(100);
  verify(await geometry());
  assert.deepEqual(errors, []);
  await writeFile('.cache/timer-layout-results.json', JSON.stringify(report, null, 2));
  console.log(
    'PASS: requested sizes, full-group geometry, compact restore, state/time variants, arrangements, menus, long names/counters, tick stability and emulated DPR 1/1.25/1.5.'
  );
  console.log(
    JSON.stringify(
      report.map((g) => ({
        client: g.viewport,
        dial: g.dial.w,
        font: g.time.font,
        play: g.play.w,
        compact: g.compact,
      })),
      null,
      2
    )
  );
} finally {
  socket?.close();
  chrome.kill();
  await sleep(200);
  const resolved = path.resolve(profile);
  assert.ok(resolved.startsWith(path.resolve(tmpdir()) + path.sep + 'pomotroid-ui-'));
  await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 150 });
}
