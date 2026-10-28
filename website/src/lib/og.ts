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

const green = '#46d39a';
const bg = '#0b0c0b';

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
        color: '#f1f0ea',
        fontFamily: 'Geist',
        backgroundColor: bg,
        backgroundImage: `radial-gradient(circle at 88% -12%, rgba(70,211,154,0.24), rgba(11,12,11,0) 55%), radial-gradient(rgba(241,240,234,0.14) 1.2px, transparent 1.4px)`,
        backgroundSize: '100% 100%, 28px 28px',
      },
    },
    h(
      'div',
      { style: { display: 'flex', alignItems: 'center', gap: 18 } },
      mark(44),
      h('span', { style: { fontSize: 36, fontWeight: 650, letterSpacing: '-0.035em', lineHeight: 1 } }, 'zgx'),
      eyebrow
        ? h('span', { style: { fontSize: 28, color: 'rgba(241,240,234,0.5)', marginLeft: 6 } }, `/  ${eyebrow}`)
        : null,
    ),
    ...children,
    h(
      'div',
      { style: { display: 'flex', marginTop: 'auto', fontFamily: 'Geist Mono', fontSize: 24, color: 'rgba(241,240,234,0.45)' } },
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
        { style: { display: 'flex', fontSize: size(title, 88, 68, 24), fontWeight: 600, letterSpacing: '-0.04em', lineHeight: 1.05, marginTop: 88, maxWidth: 1020 } },
        title,
      ),
      description
        ? h(
            'div',
            { style: { display: 'flex', fontSize: 32, lineHeight: 1.4, color: 'rgba(241,240,234,0.62)', marginTop: 30, maxWidth: 980 } },
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
        { style: { display: 'flex', flexWrap: 'wrap', fontSize: 62, fontWeight: 600, letterSpacing: '-0.04em', lineHeight: 1.1, marginTop: 44, maxWidth: 1040 } },
        h('span', {}, 'Native interfaces that do no work when nothing changes.'),
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
            h('span', { style: { fontSize: 50, fontWeight: 600, letterSpacing: '-0.045em', color: '#f1f0ea', lineHeight: 1 } }, s.value),
            h('span', { style: { fontSize: 21, color: 'rgba(241,240,234,0.6)', marginTop: 8 } }, s.label),
          ),
        ),
      ),
    ]),
    { width: 1200, height: 630, format: 'png', fonts },
  );
}
