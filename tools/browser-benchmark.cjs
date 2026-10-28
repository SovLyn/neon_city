'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:4173/';
const evidenceDir = path.resolve(process.env.EVIDENCE_DIR || 'docs/evidence');
const sampleSeconds = Number(process.env.SAMPLE_SECONDS || 60);
const warmupSeconds = 10;
let browser;
(async () => {
  if (!Number.isFinite(sampleSeconds) || sampleSeconds <= 0) throw new Error('SAMPLE_SECONDS must be a positive number');
  fs.mkdirSync(evidenceDir, { recursive: true });
  let page;
  let result;
  try {
    browser = await chromium.launch({ headless: true, ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}), args: ['--enable-unsafe-webgpu'] });
    page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
    await page.goto(baseUrl, { waitUntil: 'load' });
    await page.waitForFunction(() => window.__neonMetrics?.frame > 5 || window.__neonMetrics?.fatal_error, null, { timeout: 60000 });
    const startup = await page.evaluate(() => ({ metrics: window.__neonMetrics, ua: navigator.userAgent, adapter: navigator.gpu ? null : 'WebGPU unavailable' }));
    if (startup.metrics?.fatal_error) throw new Error(`fatal startup: ${startup.metrics.fatal_error}`);
    result = {
      ua: startup.ua,
      method: 'Headless Chromium; stationary default camera; live animation. RAF interval timings are NOT GPU timings.',
      warmupSeconds, requestedSampleSeconds: sampleSeconds, tiers: [],
    };
    for (const tier of ['Balanced', 'Performance']) {
      if (tier === 'Performance') await page.locator('#quality').click();
      const tierStart = Date.now();
      await page.waitForTimeout(warmupSeconds * 1000);
      const raw = await page.evaluate(duration => new Promise(resolve => {
        const samples = [];
        let start; let previous;
        function tick(time) {
          if (start === undefined) start = time;
          if (previous !== undefined) samples.push({ intervalMs: time - previous, cpuMs: window.__neonMetrics?.cpu_ms });
          previous = time;
          if (time - start >= duration) resolve(samples); else requestAnimationFrame(tick);
        }
        requestAnimationFrame(tick);
      }), sampleSeconds * 1000);
      if (!raw.length) throw new Error(`${tier}: no RAF samples collected`);
      const intervals = raw.map(s => s.intervalMs).sort((a, b) => a - b);
      const percentile = q => intervals[Math.min(intervals.length - 1, Math.floor(q * intervals.length))];
      result.tiers.push({
        tier, samples: raw.length, warmupSeconds: (Date.now() - tierStart - sampleSeconds * 1000) / 1000,
        actualSampleSeconds: (intervals.reduce((sum, n) => sum + n, 0) / 1000),
        medianIntervalMs: percentile(.5), p95IntervalMs: percentile(.95), p99IntervalMs: percentile(.99),
        meanIntervalMs: intervals.reduce((sum, n) => sum + n, 0) / intervals.length,
        meanCpuMs: raw.reduce((sum, s) => sum + (Number.isFinite(s.cpuMs) ? s.cpuMs : 0), 0) / raw.length,
        finalMetrics: await page.evaluate(() => window.__neonMetrics),
      });
    }
    fs.writeFileSync(path.join(evidenceDir, 'browser-benchmark.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify(result, null, 2));
  } finally {
    if (page) await page.close();
    if (browser) await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
