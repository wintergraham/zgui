// Code highlighted and framed at build time for Astro pages (the docs'
// Markdown gets the same through astro.config.mjs's rehype plugins).
import { createHighlighter } from 'shiki';
import { transformerNotationDiff, transformerNotationHighlight } from '@shikijs/transformers';
import { toHtml } from 'hast-util-to-html';
import type { Element, Root } from 'hast';
import { codeThemes } from './code-themes.mjs';
import { frame, tabs } from './code-frame.mjs';

const highlighter = await createHighlighter({
  themes: [codeThemes.light, codeThemes.dark],
  langs: ['rust', 'luau', 'sh', 'json', 'toml', 'text'],
});

export type Snippet = { code: string; lang: string; title?: string };

function pre({ code, lang, title }: Snippet): Element {
  const root = highlighter.codeToHast(code.replace(/\n+$/, ''), {
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
export function codeBlock(snippet: Snippet): string {
  return toHtml(frame(pre(snippet)) as Element);
}

/** Blocks under tabs named by their titles, as HTML. */
export function codeTabs(snippets: Snippet[], id: string): string {
  return toHtml(tabs(snippets.map(pre), id) as Element);
}
