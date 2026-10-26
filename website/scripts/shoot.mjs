// Screenshots of the built site, desktop and mobile, light and dark:
// `node scripts/shoot.mjs /rust /luau/reference/state` (serve dist first).
import { chromium } from '@playwright/test';
import fs from 'node:fs';

const base = process.env.BASE ?? 'http://localhost:4321';
const paths = process.argv.slice(2).length ? process.argv.slice(2) : ['/'];
const themes = (process.env.THEMES ?? 'light,dark').split(',');
const sizes = { desktop: { width: 1440, height: 900 }, mobile: { width: 390, height: 844 } };
const only = process.env.SIZES?.split(',') ?? Object.keys(sizes);
fs.mkdirSync('screenshots', { recursive: true });
const browser = await chromium.launch();
for (const theme of themes) {
  for (const size of only) {
    const context = await browser.newContext({ viewport: sizes[size], colorScheme: theme, deviceScaleFactor: 1 });
    const page = await context.newPage();
    page.on('pageerror', (e) => console.log('pageerror', e.message));
    page.on('console', (m) => m.type() === 'error' && console.log('console', m.text()));
    for (const p of paths) {
      await page.goto(base + p, { waitUntil: 'networkidle' });
      const name = `screenshots/${(p.replace(/\//g, '_') || '_') }-${size}-${theme}.png`;
      await page.screenshot({ path: name, fullPage: process.env.FULL === '1' });
      console.log(name);
    }
    await context.close();
  }
}
await browser.close();
