// Headless screenshot of the Evolution Studio.
//
// Serves the prebuilt `build/` directory via a tiny static server,
// loads /studio in Chromium, and writes a PNG. Used to capture the
// rendered Studio state for the .proofs/ report — no human in the
// loop.

import { chromium } from '@playwright/test';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { existsSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const BUILD_DIR = resolve('build');
const PORT = Number(process.env.STUDIO_PREVIEW_PORT ?? 4319);
// SvelteKit adapter-static + GENESIS_BASE_PATH defaults to '/genesis'.
// We respect the build-time setting so we hit /genesis/studio.
const BASE = process.env.GENESIS_BASE_PATH ?? '/genesis';
const STUDIO_PATH = `${BASE}/studio`;

if (!existsSync(BUILD_DIR)) {
  console.error('!! build/ missing — run `vite build` first.');
  process.exit(1);
}

const mime = (p) => {
  if (p.endsWith('.html')) return 'text/html; charset=utf-8';
  if (p.endsWith('.js')) return 'application/javascript; charset=utf-8';
  if (p.endsWith('.css')) return 'text/css; charset=utf-8';
  if (p.endsWith('.svg')) return 'image/svg+xml';
  if (p.endsWith('.png')) return 'image/png';
  if (p.endsWith('.json')) return 'application/json; charset=utf-8';
  if (p.endsWith('.woff2')) return 'font/woff2';
  return 'application/octet-stream';
};

const server = createServer(async (req, res) => {
  // Strip query string + base path for filesystem lookup.
  let url = (req.url ?? '/').split('?')[0];
  if (url.startsWith(BASE)) url = url.slice(BASE.length);
  if (url === '' || url === '/') url = '/index.html';

  let fsPath = join(BUILD_DIR, decodeURIComponent(url));
  try {
    if (existsSync(fsPath) && statSync(fsPath).isFile()) {
      res.writeHead(200, { 'Content-Type': mime(fsPath) });
      res.end(await readFile(fsPath));
      return;
    }
    // SPA fallback — adapter-static wrote index.html.
    const fallback = join(BUILD_DIR, 'index.html');
    if (existsSync(fallback)) {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(await readFile(fallback));
      return;
    }
    res.writeHead(404);
    res.end('not found');
  } catch (err) {
    res.writeHead(500);
    res.end(String(err));
  }
});

await new Promise((r) => server.listen(PORT, '127.0.0.1', r));
console.log(`[screenshot] static server up at http://127.0.0.1:${PORT}${STUDIO_PATH}`);

let browser;
try {
  browser = await chromium.launch();
  const ctx = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
  });
  const page = await ctx.newPage();
  // Mute console noise from missing favicon etc.
  page.on('console', (msg) => {
    const t = msg.type();
    if (t === 'error' || t === 'warning') {
      console.log(`[browser:${t}] ${msg.text()}`);
    }
  });
  const url = `http://127.0.0.1:${PORT}${STUDIO_PATH}`;
  console.log(`[screenshot] loading ${url}`);
  await page.goto(url, { waitUntil: 'networkidle', timeout: 30_000 });
  // Studio uses VITE_TEMPER_API_BASE for live; we are offline so it
  // will fall back to fixture automatically.
  // Give Svelte a tick to settle reactive state.
  await page.waitForTimeout(800);

  const fullshot = process.env.STUDIO_SCREENSHOT_PATH ?? 'studio-screenshot.png';
  await page.screenshot({ path: fullshot, fullPage: true });
  console.log(`[screenshot] wrote ${fullshot}`);

  // The default screenshot lands on the auto-selected Live episode
  // (the celebration banner is what we want as the hero shot).
  // Now click the OTHER episode — the AwaitingApproval one — to
  // capture the full bracket with all 5 variants and their PASS/KILL
  // mix.
  const otherEpisode = page.locator('button:has-text("downvote low-quality")');
  if (await otherEpisode.count()) {
    await otherEpisode.first().click();
    await page.waitForTimeout(400);
    const bracketShot = 'studio-screenshot-bracket.png';
    await page.screenshot({ path: bracketShot, fullPage: true });
    console.log(`[screenshot] wrote ${bracketShot}`);

    // Click one of the KILLED cells to populate the inspector with
    // a counterexample — that's the demo-worthy state.
    const killedCell = page.locator('button[aria-label*="KILLED"]').first();
    if (await killedCell.count()) {
      await killedCell.click();
      await page.waitForTimeout(300);
      await page.screenshot({ path: 'studio-screenshot-evidence.png', fullPage: true });
      console.log('[screenshot] wrote studio-screenshot-evidence.png');
    }
  }
} finally {
  if (browser) await browser.close();
  server.close();
}
