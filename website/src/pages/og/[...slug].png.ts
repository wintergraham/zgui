import type { APIRoute, GetStaticPaths } from 'astro';
import { source } from '@/lib/source';
import { docsImage, homeImage } from '@/lib/og';
import { headline } from '@/lib/perf';
import { sets } from '../../../docs.manifest.mjs';

export const getStaticPaths = (() => [
  { params: { slug: 'index' }, props: { home: true } },
  ...source.getPages().map((page) => ({
    params: { slug: page.slugs.join('/') },
    props: { slugs: page.slugs },
  })),
]) satisfies GetStaticPaths;

export const GET: APIRoute = ({ props }) => {
  if (props.home) return homeImage(headline().cards.map((c) => ({ value: c.value, label: c.label })));
  const page = source.getPage(props.slugs)!;
  const set = sets.find((s) => s.dir === props.slugs[0])!;
  return docsImage(set.title, page.data.title, page.data.description);
};
