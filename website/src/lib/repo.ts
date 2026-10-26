// Facts read from the repository's source at build time, so the pages that
// show them can't drift from the code.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(process.cwd(), '..');
const read = (file: string) => fs.readFileSync(path.join(root, file), 'utf8');

/** A Rust `const NAME: &str = "…";`, unescaped. */
export function rustString(file: string, name: string): string {
  const match = read(file).match(new RegExp(`const ${name}: &str = "((?:[^"\\\\]|\\\\.)*)";`));
  if (!match) throw new Error(`${file}: no const ${name}: &str`);
  return match[1].replace(/\\(["\\n])/g, (_, c) => (c === 'n' ? '\n' : c));
}

/** `zgx lint`'s rules: `zgx_script::lint::RULES`. */
export function lintRules(): { name: string; summary: string }[] {
  const source = read('crates/zgx-script/src/lint.rs');
  const table = source.slice(source.indexOf('pub const RULES'), source.indexOf('];', source.indexOf('pub const RULES')));
  const rules = [...table.matchAll(/\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)",?\s*\)/g)].map((m) => ({
    name: m[1],
    summary: m[2].replace(/\\"/g, '"'),
  }));
  if (!rules.length) throw new Error('crates/zgx-script/src/lint.rs: no RULES');
  return rules;
}

/** The version every published crate carries: `[workspace.package] version`. */
export function workspaceVersion(): string {
  const toml = read('Cargo.toml');
  const block = toml.slice(toml.indexOf('[workspace.package]'));
  const match = block.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error('Cargo.toml: no [workspace.package] version');
  return match[1];
}

/**
 * What crates.io serves of zgx now (it can trail the repository's version,
 * between a release's tag and its publishing), or null when it can't be asked.
 */
export async function publishedVersion(): Promise<string | null> {
  try {
    const answer = await fetch('https://crates.io/api/v1/crates/zgx', {
      headers: { 'User-Agent': 'zgx.zeron.sh build (github.com/zeronsh/zgx)' },
      signal: AbortSignal.timeout(8000),
    });
    if (!answer.ok) return null;
    const data = (await answer.json()) as { crate?: { max_stable_version?: string } };
    return data.crate?.max_stable_version ?? null;
  } catch {
    return null;
  }
}
