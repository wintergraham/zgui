import type { APIRoute, GetStaticPaths } from 'astro';
import { source } from '@/lib/source';
import { pageMarkdown } from '@/lib/llms';

export const getStaticPaths = (() =>
  source.getPages().map((page) => ({ params: { slug: page.slugs.join('/') }, props: { slugs: page.slugs } }))) satisfies GetStaticPaths;

export const GET: APIRoute = ({ props }) =>
  new Response(pageMarkdown(source.getPage(props.slugs)!), { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
