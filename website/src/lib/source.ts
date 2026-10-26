import * as path from 'node:path';
import { type CollectionEntry, getCollection } from 'astro:content';
import { loader, type StaticSource } from 'fumadocs-core/source';
import { lucideIconsPlugin } from 'fumadocs-core/source/plugins/lucide-icons';
import { structure, type StructuredData } from 'fumadocs-core/mdx-plugins';

const relative = (file?: string) => path.relative('content/docs', file!);

type PageData = CollectionEntry<'docs'>['data'] & { _raw: CollectionEntry<'docs'> };
type MetaData = CollectionEntry<'meta'>['data'];

async function files(): Promise<StaticSource<{ pageData: PageData; metaData: MetaData }>> {
  const out: StaticSource<{ pageData: PageData; metaData: MetaData }> = { files: [] };
  for (const page of await getCollection('docs'))
    out.files.push({ type: 'page', path: relative(page.filePath), data: { ...page.data, _raw: page } });
  for (const meta of await getCollection('meta'))
    out.files.push({ type: 'meta', path: relative(meta.filePath), data: meta.data });
  return out;
}

export const source = loader({
  source: await files(),
  baseUrl: '/',
  plugins: [lucideIconsPlugin()],
});

// The same for every page: serialized once, icons rendered to HTML here so
// no icon library ships to the browser.
export const pageTree = await source.serializePageTree(source.getPageTree());

export function getStructuredData(entry: CollectionEntry<'docs'>): StructuredData {
  return structure(entry.body ?? '');
}
