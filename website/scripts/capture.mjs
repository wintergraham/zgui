// Captures for design review, small enough to commit:
//   node scripts/capture.mjs design/screenshots/after
// Home, a Rust doc, the Luau reference, performance and a code-heavy page;
// 1440 and 390 wide; light and dark; the top 2,400 px of each, as JPEG.
// (Serve dist first: npx astro preview.)
import { chromium } from '@playwright/test';
import fs from 'node:fs';

const [dir = 'design/screenshots/now', ...rest] = process.argv.slice(2);
const pages = rest.length ? rest : ['/', '/rust/layout', '/luau/reference/state', '/performance', '/rust/composition'];
const base = process.env.BASE ?? 'http://localhost:4321';
fs.mkdirSync(dir, { recursive: true });
const browser = await chromium.launch();
for (const theme of ['light', 'dark']) {
  for (const width of [1440, 390]) {
    // A viewport as tall as the capture, so sticky chrome shows as it does on screen.
    const context = await browser.newContext({ viewport: { width, height: 2400 }, colorScheme: theme, deviceScaleFactor: 1 });
    const page = await context.newPage();
    for (const p of pages) {
      await page.goto(base + p, { waitUntil: 'networkidle' });
      // Scroll through, so what animates into view does, then let it settle.
      await page.evaluate(async () => {
        for (let y = 0; y < document.documentElement.scrollHeight; y += 600) {
          window.scrollTo(0, y);
          await new Promise((r) => setTimeout(r, 60));
        }
        window.scrollTo(0, 0);
      });
      await page.waitForTimeout(1600);
      const height = Math.min(2400, await page.evaluate(() => document.documentElement.scrollHeight));
      const name = `${dir}/${p === '/' ? 'home' : p.slice(1).replace(/\//g, '_')}-${width}-${theme}.jpg`;
      await page.screenshot({ path: name, type: 'jpeg', quality: 72, clip: { x: 0, y: 0, width, height } });
    }
    await context.close();
  }
}
await browser.close();
console.log(`captured ${pages.length * 4} into ${dir}`);
