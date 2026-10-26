// Full-page captures for design review: node scripts/capture.mjs <dir> [paths…]
// 1440, 1024 and 390 wide, light and dark (serve dist first: npx astro preview).
import { chromium } from '@playwright/test';
import fs from 'node:fs';

const [dir = 'design/now', ...rest] = process.argv.slice(2);
const paths = rest.length ? rest : ['/', '/rust/layout', '/luau/reference/state', '/performance', '/rust/composition'];
const base = process.env.BASE ?? 'http://localhost:4321';
fs.mkdirSync(dir, { recursive: true });
const browser = await chromium.launch();
for (const theme of ['light', 'dark']) {
  for (const width of [1440, 1024, 390]) {
    const context = await browser.newContext({ viewport: { width, height: 900 }, colorScheme: theme });
    const page = await context.newPage();
    page.on('pageerror', (e) => console.log('pageerror', e.message));
    for (const p of paths) {
      await page.goto(base + p, { waitUntil: 'networkidle' });
      const name = `${dir}/${(p === '/' ? 'home' : p.slice(1).replace(/\//g, '_'))}-${width}-${theme}.png`;
      await page.screenshot({ path: name, fullPage: true });
    }
    await context.close();
  }
}
await browser.close();
console.log(`captured ${paths.length * 6} into ${dir}`);
