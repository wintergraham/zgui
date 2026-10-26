// Every colour of the code themes reads at WCAG AA (4.5:1) on its background.
import { zgxLight, zgxDark } from '../src/lib/code-themes.mjs';

const lum = (hex) => {
  const [r, g, b] = hex.slice(1, 7).match(/../g).map((x) => parseInt(x, 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
let bad = 0;
for (const t of [zgxLight, zgxDark]) {
  // The block's own background, and the header's slightly different one.
  for (const bg of [t.bg, t.type === 'light' ? '#f7f7f7' : '#161616']) {
    for (const color of new Set(t.tokenColors.map((r) => r.settings.foreground))) {
      const r = ratio(color, bg);
      if (r < 4.5) bad++;
      console.log(`${t.name} ${bg} ${color} ${r.toFixed(2)}${r < 4.5 ? '  FAIL' : ''}`);
    }
  }
}
process.exit(bad ? 1 : 0);
