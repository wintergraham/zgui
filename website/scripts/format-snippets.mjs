// Formats the Luau snippets of the repository's docs with stylua and the
// repository's stylua.toml, in place: node scripts/format-snippets.mjs [--check]
// (STYLUA=path/to/stylua). Snippets stylua can't parse (fragments) are left
// as they are and listed.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const root = path.resolve(import.meta.dirname, '../..');
const stylua = process.env.STYLUA ?? 'stylua';
const check = process.argv.includes('--check');
const files = ['README.md', ...['docs', 'docs/reference', 'docs/agents'].flatMap((d) =>
  fs.readdirSync(path.join(root, d)).filter((f) => f.endsWith('.md')).map((f) => `${d}/${f}`))];

let changed = 0;
const skipped = [];
for (const file of files) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  let edits = 0;
  const out = text.replace(/^```(lua|luau)\n([\s\S]*?)^```$/gm, (block, lang, code, offset) => {
    // A snippet quoting a diagnostic's line:column counts its own spaces.
    if (/:\d+:\d+: (error|warning)/.test(code)) return block;
    let formatted;
    try {
      formatted = execFileSync(stylua, ['--config-path', path.join(root, 'stylua.toml'), '--stdin-filepath', 'snippet.luau', '-'], {
        input: code,
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
      });
    } catch {
      skipped.push(`${file}:${text.slice(0, offset).split('\n').length}`);
      return block;
    }
    if (formatted === code) return block;
    edits++;
    return `\`\`\`${lang}\n${formatted}\`\`\``;
  });
  if (edits) {
    changed += edits;
    console.log(`${file}: ${edits} snippet(s) ${check ? 'would be ' : ''}formatted`);
    if (!check) fs.writeFileSync(path.join(root, file), out);
  }
}
console.log(`${changed} formatted; ${skipped.length} not parsed (left as written): ${skipped.join(' ')}`);
process.exit(check && changed ? 1 : 0);
