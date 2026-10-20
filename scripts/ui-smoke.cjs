// Development check against the native Tauri WebView (CDP port 9223).
const { chromium } = require('../tools/dev/node_modules/playwright');
const assert = require('node:assert/strict');
const path = require('node:path');
(async () => {
  const browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
  const page = browser.contexts().flatMap(context => context.pages()).find(page => /tauri|localhost/.test(page.url()));
  assert(page, 'Native Tauri page missing');
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.waitForFunction(() => document.getElementById('token').value.length > 20);
  const initial = await page.evaluate(() => window.__TAURI__.core.invoke('snapshot'));
  assert.equal(initial.running, false);
  await page.locator('#go-connection').click();
  assert.equal(await page.locator('#connection').isVisible(), true);
  assert((await page.locator('#connection h1').boundingBox()).y >= 0);
  await page.locator('[name=mode][value=frp]').check();
  assert.equal(await page.locator('#frp-settings').isVisible(), true);
  await page.locator('[name=mode][value=cloudflare]').check();
  assert.equal(await page.locator('#lt-settings').isVisible(), true);
  assert.equal(await page.locator('#require-auth').count(),0);

  assert.equal(await page.locator('#token-field').isVisible(),true);
  assert.equal(await page.locator('#token-field').isVisible(),true);
  await page.screenshot({ path: path.resolve(__dirname, '../preview-connection.png') });
  await page.evaluate(connection => window.__TAURI__.core.invoke('save_settings', { connection }), initial.connection);
  await page.locator('[data-tab=overview]').click();
  await page.screenshot({ path: path.resolve(__dirname, '../preview-tauri.png') });
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  assert.equal(overflow, false, 'Horizontal overflow');
  if (initial.folders.length === 0) {
    await page.locator('#toggle-server').click();
    await page.waitForFunction(() => document.getElementById('toast').textContent.includes('dossier'));
    assert.equal((await page.evaluate(() => window.__TAURI__.core.invoke('snapshot'))).running, false);
  }
  assert.deepEqual(errors, []);
  console.log('Native Tauri UI: IPC, tabs, tunnel modes, validation and layout passed.');
  await browser.close();
})().catch(error => { console.error(error); process.exitCode = 1; });
