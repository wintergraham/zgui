import type { APIRoute } from 'astro';
import { createFromSource } from 'fumadocs-core/search/server';
import { getStructuredData, source } from '@/lib/source';

// Records of evidence: found by their headings and each section's first
// paragraph, so their tables don't make every search download a megabyte.
const brief = new Set(['docs/validation.md', 'docs/benchmarking.md']);

function structured(page: (typeof source)['$inferPage']) {
  const data = getStructuredData(page.data._raw);
  if (!brief.has(page.data.source ?? '')) return data;
  const seen = new Set<string | undefined>();
  return {
    headings: data.headings,
    contents: data.contents.filter((c) => (seen.has(c.heading) ? false : (seen.add(c.heading), true))),
  };
}

// The static search index: one JSON file, searched in the browser.
const server = createFromSource(source, {
  language: 'english',
  // Results are ranked by relevance; no property is sorted, so no sorting index ships.
  sort: { enabled: false },
  buildIndex(page) {
    return {
      id: page.url,
      title: page.data.title,
      description: page.data.description,
      structuredData: structured(page),
      url: page.url,
    };
  },
});

export const GET: APIRoute = () => server.staticGET();
