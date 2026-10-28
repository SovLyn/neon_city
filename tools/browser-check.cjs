'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:4173/';
const evidenceDir = path.resolve(process.env.EVIDENCE_DIR || 'docs/evidence');
const views = [
  ['showcase', 'camera=showcase&time=3&paused=1'],
  ['no-bloom', 'camera=showcase&time=3&paused=1&bloom=0'],
  ['no-atmosphere', 'camera=showcase&time=3&paused=1&atmosphere=0'],
  ['reflection', 'camera=showcase&time=3&paused=1&debug=reflection'],
  ['roughness', 'camera=showcase&time=3&paused=1&debug=roughness'],
  ['normal', 'camera=showcase&time=3&paused=1&debug=normal'],
];
const results = [];
let browser;
let failed = false;
const recordError = (result, error) => { result.errors.push(String(error)); result.pass = false; };

(async () => {
  fs.mkdirSync(evidenceDir, { recursive: true });
  try {
    browser = await chromium.launch({ headless: true, ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}), args: ['--enable-unsafe-webgpu'] });
    for (const [name, query] of views) {
      let page;
      const result = { name, query, errors: [], pass: false };
      results.push(result);
      try {
        page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
        page.on('pageerror', e => recordError(result, e));
        page.on('console', m => { if (m.type() === 'error') recordError(result, m.text()); });
        await page.goto(new URL(`?${query}`, baseUrl).href, { waitUntil: 'load' });
        await page.waitForFunction(() => window.__neonMetrics?.frame > 30 || window.__neonMetrics?.fatal_error, null, { timeout: 60000 });
        result.metrics = await page.evaluate(() => window.__neonMetrics);
        if (!result.metrics || result.metrics.fatal_error) throw new Error(`fatal startup: ${result.metrics?.fatal_error || 'metrics missing'}`);
        await page.screenshot({ path: path.join(evidenceDir, `${name}.png`) });
        result.pass = !result.errors.length;
        if (!result.pass) failed = true;
      } catch (e) { recordError(result, e); failed = true; }
      finally { if (page) try { await page.close(); } catch (e) { recordError(result, `page close: ${e}`); } }
    }

    for (const mode of ['quality-resize-pause', 'no-adapter', 'missing-atlas', 'device-destroy']) {
      let page;
      const result = { mode, errors: [], pass: false };
      results.push(result);
      try {
        page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
        page.on('pageerror', e => recordError(result, e));
        page.on('console', m => { if (m.type() === 'error') recordError(result, m.text()); });
        if (mode === 'no-adapter') await page.addInitScript(() => { navigator.gpu.requestAdapter = async () => null; });
        if (mode === 'device-destroy') await page.addInitScript(() => {
          const configure = GPUCanvasContext.prototype.configure;
          GPUCanvasContext.prototype.configure = function (config) { window.__checkDevice = config.device; return configure.call(this, config); };
        });
        if (mode === 'missing-atlas') await page.route('**/signs-rgba.png', route => route.abort());
        await page.goto(baseUrl, { waitUntil: 'load' });
        await page.waitForFunction(() => window.__neonMetrics?.frame > 10 || window.__neonMetrics?.fatal_error, null, { timeout: 60000 });
        if (mode === 'quality-resize-pause') {
          const before = await page.evaluate(() => window.__neonMetrics);
          if (before.fatal_error) throw new Error(`unexpected fatal error: ${before.fatal_error}`);
          await page.locator('#quality').click();
          await page.waitForFunction(() => window.__neonMetrics?.quality === 'Performance', null, { timeout: 10000 });
          await page.setViewportSize({ width: 1280, height: 800 });
          await page.waitForFunction(() => window.__neonMetrics?.output?.[0] === 1280, null, { timeout: 10000 });
          await page.locator('#pause').click();
          await page.waitForTimeout(500);
          const paused = await page.evaluate(() => window.__neonMetrics);
          await page.waitForTimeout(500);
          const stillPaused = await page.evaluate(() => window.__neonMetrics);
          if (!Number.isFinite(paused.simulation_time_s) || paused.simulation_time_s !== stillPaused.simulation_time_s) throw new Error('Simulation advanced while paused');
          if (stillPaused.frame <= paused.frame) throw new Error('Paused rendering stopped; temporal sampling must continue');
          await page.locator('#pause').click();
          await page.waitForFunction(time => window.__neonMetrics?.simulation_time_s > time, paused.simulation_time_s, { timeout: 10000 });
          result.observed = { quality: paused.quality, output: paused.output, pausedFrame: paused.frame, resumedFrame: await page.evaluate(() => window.__neonMetrics.frame) };
          result.pass = !result.errors.length;
          if (!result.pass) failed = true;
        } else {
          const before = await page.evaluate(() => window.__neonMetrics);
          if (mode === 'device-destroy') {
            if (before.fatal_error) throw new Error(`unexpected fatal before device destroy: ${before.fatal_error}`);
            await page.evaluate(() => window.__checkDevice.destroy());
          } else if (!before.fatal_error) {
            throw new Error('expected fatal failure was not observed');
          }
          await page.waitForFunction(() => window.__neonMetrics?.fatal_error, null, { timeout: 10000 });
          const fatal = await page.evaluate(() => window.__neonMetrics);
          await page.waitForTimeout(500);
          const after = await page.evaluate(() => window.__neonMetrics);
          if (after.frame !== fatal.frame) throw new Error('RAF continued after fatal error');
          result.observed = { before, fatal, after };
          // Runtime errors during deliberately induced fatal paths are recorded, but the
          // acceptance criterion is the fatal metric and stopped RAF, not a message string.
          result.pass = true;
        }
      } catch (e) { recordError(result, e); failed = true; }
      finally { if (page) try { await page.close(); } catch (e) { recordError(result, `page close: ${e}`); } }
    }
  } catch (e) { failed = true; results.push({ mode: 'browser', pass: false, errors: [String(e)] }); }
  finally {
    if (browser) try { await browser.close(); } catch (e) { failed = true; results.push({ mode: 'browser-close', pass: false, errors: [String(e)] }); }
    try { fs.writeFileSync(path.join(evidenceDir, 'browser-check.json'), JSON.stringify(results, null, 2)); }
    catch (e) { console.error(`Could not save browser-check results: ${e}`); failed = true; }
  }
  console.log(JSON.stringify(results, null, 2));
  if (failed || results.some(r => !r.pass)) process.exitCode = 1;
})();
