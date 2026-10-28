// Code highlighted and framed at build time for Astro pages (the docs'
// Markdown gets the same through astro.config.mjs's rehype plugins).
import { createHighlighter, type Highlighter } from 'shiki';
import { transformerNotationDiff, transformerNotationHighlight } from '@shikijs/transformers';
import { toHtml } from 'hast-util-to-html';
import type { Element, Root } from 'hast';
import { codeThemes } from './code-themes.mjs';
import { frame, tabs } from './code-frame.mjs';

// Made on first use rather than as the module loads: a top-level await here
// runs inside MDX's module graph, where a failure surfaces only as Astro's
// "error while rendering". A failure is said, and the next call retries.
let started: Promise<Highlighter> | undefined;
function highlighter(): Promise<Highlighter> {
  started ??= createHighlighter({
    themes: [codeThemes.light, codeThemes.dark],
    langs: ['rust', 'luau', 'sh', 'json', 'toml'],
  }).catch((error) => {
    console.error('highlight: Shiki did not start', error);
    started = undefined;
    throw error;
  });
  return started;
}

export type Snippet = { code: string; lang: string; title?: string };

async function pre({ code, lang, title }: Snippet): Promise<Element> {
  const root = (await highlighter()).codeToHast(code.replace(/\n+$/, ''), {
    lang,
    themes: { light: 'zgx-light', dark: 'zgx-dark' },
    defaultColor: false,
    transformers: [
      transformerNotationHighlight(),
      transformerNotationDiff(),
      {
        code(node) {
          this.addClassToHast(node, `language-${lang}`);
        },
      },
    ],
  }) as Root;
  const element = root.children.find((c): c is Element => c.type === 'element' && c.tagName === 'pre')!;
  if (title) element.properties.title = title;
  return element;
}

/** One framed block, as HTML. */
export async function codeBlock(snippet: Snippet): Promise<string> {
  return toHtml(frame(await pre(snippet)) as Element);
}

/** Blocks under tabs named by their titles, as HTML. */
export async function codeTabs(snippets: Snippet[], id: string): Promise<string> {
  return toHtml(tabs(await Promise.all(snippets.map(pre)), id) as Element);
}
