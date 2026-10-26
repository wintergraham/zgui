export const siteName = 'zgx';
export const siteUrl = 'https://zgx.zeron.sh';
export const repoUrl = 'https://github.com/zeronsh/zgx';

/** The Open Graph image of a page: `/og/rust/layout.png`, `/og/index.png` for the home page. */
export function ogImage(slugs: string[]): string {
  return `/og/${slugs.length ? slugs.join('/') : 'index'}.png`;
}

/** The Markdown of a docs page, for agents and "copy as Markdown": `/rust/layout.md`. */
export function markdownUrl(url: string): string {
  return `${url}.md`;
}
