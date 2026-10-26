// zgx-script's API as luau-lsp checks it: the members of `declare ui` and
// `declare anime` in crates/zgx-script/types/zgx.d.luau, their types and
// their comments, each linked to the reference page that covers it.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(process.cwd(), '..');
export const typesFile = 'crates/zgx-script/types/zgx.d.luau';

export type Member = { name: string; type: string; doc: string; group: string };

/** The top-level members of `declare <global>: { … }`. */
function members(lines: string[], global: string): Member[] {
  const start = lines.findIndex((l) => l.startsWith(`declare ${global}: {`));
  if (start === -1) throw new Error(`${typesFile}: no declare ${global}`);
  const out: Member[] = [];
  let doc: string[] = [];
  for (let i = start + 1; i < lines.length && !lines[i].startsWith('}'); i++) {
    const line = lines[i];
    const comment = line.match(/^\t--\s?(.*)$/);
    if (comment) {
      doc.push(comment[1]);
      continue;
    }
    const head = line.match(/^\t(\[?"?[A-Za-z_][\w]*"?\]?)\??:\s?(.*)$/);
    if (!head) {
      if (!line.startsWith('\t\t') && line.trim() !== '') doc = [];
      continue;
    }
    // The member's type runs until its brackets close and a line ends with a comma.
    let type = head[2];
    let depth = 0;
    const count = (s: string) => {
      for (const c of s) depth += '({<['.includes(c) ? 1 : ')}>]'.includes(c) && !(c === '>' && s.includes('->')) ? -1 : 0;
    };
    count(type.replace(/->/g, ''));
    const continues = (next: string | undefined) => next !== undefined && (next.startsWith('\t\t') || /^\t[)}\]]/.test(next));
    while ((depth > 0 || !type.trimEnd().endsWith(',')) && continues(lines[i + 1])) {
      i++;
      type += ` ${lines[i].trim()}`;
      count(lines[i].replace(/->/g, ''));
    }
    out.push({ name: head[1].replace(/[[\]"]/g, ''), type: type.trim().replace(/,$/, '').replace(/,\s*([}\])])/g, ' $1'), doc: doc.join(' '), group: global });
    doc = [];
  }
  return out;
}

export function luauApi(): Member[] {
  const lines = fs.readFileSync(path.join(root, typesFile), 'utf8').split('\n');
  return [...members(lines, 'ui'), ...members(lines, 'anime')];
}

/** The reference page that says `ui.name` most, its headings counting most. */
export function referenceFor(): (m: Member) => string | undefined {
  const dir = path.join(process.cwd(), 'content/docs/luau');
  const pages: { url: string; text: string }[] = [];
  const walk = (d: string) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const full = path.join(d, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.name.endsWith('.md')) {
        const rel = path.relative(dir, full).replace(/\.md$/, '').replace(/(^|\/)index$/, '');
        pages.push({ url: `/luau${rel ? `/${rel}` : ''}`, text: fs.readFileSync(full, 'utf8') });
      }
    }
  };
  walk(dir);
  return (m) => {
    const needle = `${m.group}.${m.name}`;
    let best: { url: string; score: number } | undefined;
    for (const p of pages) {
      const inHeadings = p.text.split('\n').filter((l) => l.startsWith('#') && l.includes(m.name)).length;
      const uses = p.text.split(needle).length - 1;
      const score = inHeadings * 20 + uses + (p.url.includes('/reference/') ? 10 : 0);
      if (uses + inHeadings > 0 && (!best || score > best.score)) best = { url: p.url, score };
    }
    return best?.url;
  };
}
