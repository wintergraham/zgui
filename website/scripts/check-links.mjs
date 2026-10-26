// Every internal link and anchor in the built site resolves: run after
// `astro build` (part of `npm run build`).
import fs from 'node:fs';
import path from 'node:path';

const dist = path.resolve(process.argv[2] ?? 'dist');
const files = [];
const walk = (dir) => {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full);
    else if (entry.name.endsWith('.html')) files.push(full);
  }
};
walk(dist);

const ids = new Map();
const html = new Map();
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  html.set(file, text);
  ids.set(file, new Set([...text.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])));
}
const target = (url) => {
  const clean = decodeURI(url.split('#')[0].split('?')[0]);
  if (clean === '/' || clean === '') return path.join(dist, 'index.html');
  const candidates = [path.join(dist, clean), path.join(dist, `${clean}.html`), path.join(dist, clean, 'index.html')];
  return candidates.find((c) => fs.existsSync(c) && fs.statSync(c).isFile());
};

const problems = [];
for (const [file, text] of html) {
  const page = '/' + path.relative(dist, file).replace(/\.html$/, '').replace(/(^|\/)index$/, '');
  for (const [, url] of text.matchAll(/<a\s[^>]*href="([^"]+)"/g)) {
    if (/^(https?:|mailto:|\/\/)/.test(url)) continue;
    const absolute = url.startsWith('#') ? `${page}${url}` : url.startsWith('/') ? url : null;
    if (!absolute) continue;
    const resolved = target(absolute);
    if (!resolved) {
      problems.push(`${page}: ${url}: no such page`);
      continue;
    }
    const hash = absolute.split('#')[1];
    if (hash && resolved.endsWith('.html') && !ids.get(resolved)?.has(decodeURIComponent(hash)))
      problems.push(`${page}: ${url}: no #${hash} there`);
  }
}
const unique = [...new Set(problems)];
if (unique.length) {
  console.error(`check-links: ${unique.length} broken\n${unique.map((p) => `  ${p}`).join('\n')}`);
  process.exit(1);
}
console.log(`check-links: ${files.length} pages, every internal link resolves`);
