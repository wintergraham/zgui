// Open Graph images, 1200×630 PNGs drawn at build time with Takumi.
import fs from 'node:fs';
import path from 'node:path';
import { createElement as h, type ReactNode } from 'react';
import { ImageResponse } from 'takumi-js/response';

const font = (file: string) => fs.readFileSync(path.join(process.cwd(), 'node_modules/@fontsource-variable', file));
const fonts = [
  { name: 'Geist', data: font('geist/files/geist-latin-wght-normal.woff2') },
  { name: 'Geist Mono', data: font('geist-mono/files/geist-mono-latin-wght-normal.woff2') },
];

const green = '#5cbf7e';
const bg = '#0a0a0a';

function mark(size: number) {
  return h(
    'svg',
    { width: size, height: size, viewBox: '0 0 24 24' },
    h('rect', { x: 2, y: 2, width: 20, height: 20, rx: 6, fill: green }),
    h('path', { d: 'M7.5 8h9l-9 8h9', fill: 'none', stroke: bg, strokeWidth: 2.2, strokeLinecap: 'round', strokeLinejoin: 'round' }),
  );
}

function frame(eyebrow: string, children: ReactNode[]) {
  return h(
    'div',
    {
      style: {
        display: 'flex',
        flexDirection: 'column',
        width: '100%',
        height: '100%',
        padding: '72px 80px',
        color: '#fafafa',
        fontFamily: 'Geist',
        backgroundColor: bg,
        backgroundImage: `radial-gradient(circle at 85% -10%, rgba(92,191,126,0.28), rgba(10,10,10,0) 55%), linear-gradient(rgba(255,255,255,0.045) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.045) 1px, transparent 1px)`,
        backgroundSize: '100% 100%, 48px 48px, 48px 48px',
      },
    },
    h(
      'div',
      { style: { display: 'flex', alignItems: 'center', gap: 18 } },
      mark(44),
      h('span', { style: { fontSize: 34, fontWeight: 650, letterSpacing: '-0.02em' } }, 'zgx'),
      eyebrow
        ? h('span', { style: { fontSize: 28, color: 'rgba(250,250,250,0.5)', marginLeft: 6 } }, `/  ${eyebrow}`)
        : null,
    ),
    ...children,
    h(
      'div',
      { style: { display: 'flex', marginTop: 'auto', fontFamily: 'Geist Mono', fontSize: 24, color: 'rgba(250,250,250,0.45)' } },
      'zgx.zeron.sh',
    ),
  );
}

const size = (text: string, big: number, small: number, at: number) => (text.length > at ? small : big);

/** A docs page: its doc set, title and description. */
export function docsImage(set: string, title: string, description?: string) {
  return new ImageResponse(
    frame(set, [
      h(
        'div',
        { style: { display: 'flex', fontSize: size(title, 84, 64, 26), fontWeight: 650, letterSpacing: '-0.035em', lineHeight: 1.05, marginTop: 88, maxWidth: 1000 } },
        title,
      ),
      description
        ? h(
            'div',
            { style: { display: 'flex', fontSize: 32, lineHeight: 1.4, color: 'rgba(250,250,250,0.62)', marginTop: 28, maxWidth: 980 } },
            description.length > 150 ? `${description.slice(0, 147).replace(/\s+\S*$/, '')}…` : description,
          )
        : null,
    ]),
    { width: 1200, height: 630, format: 'png', fonts },
  );
}

/** The home page: what zgx is, and its headline numbers. */
export function homeImage(stats: { value: string; label: string }[]) {
  return new ImageResponse(
    frame('', [
      h(
        'div',
        { style: { display: 'flex', fontSize: 60, fontWeight: 650, letterSpacing: '-0.04em', lineHeight: 1.05, marginTop: 44, maxWidth: 1000 } },
        'Native interfaces in Rust and Luau that do no work when nothing changes.',
      ),
      h(
        'div',
        { style: { display: 'flex', gap: 20, marginTop: 40, marginBottom: 28 } },
        ...stats.map((s) =>
          h(
            'div',
            {
              style: {
                display: 'flex',
                flexDirection: 'column',
                flex: 1,
                padding: '22px 24px',
                borderRadius: 18,
                border: '1px solid rgba(255,255,255,0.12)',
                backgroundColor: 'rgba(255,255,255,0.03)',
              },
            },
            h('span', { style: { fontSize: 46, fontWeight: 650, color: green, letterSpacing: '-0.03em' } }, s.value),
            h('span', { style: { fontSize: 21, color: 'rgba(250,250,250,0.6)', marginTop: 6 } }, s.label),
          ),
        ),
      ),
    ]),
    { width: 1200, height: 630, format: 'png', fonts },
  );
}
