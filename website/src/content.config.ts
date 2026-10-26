import { glob } from 'astro/loaders';
import { defineCollection, z } from 'astro:content';

// website/content/docs is written by scripts/sync-docs.mjs from the
// repository's docs (and content-src/ for the hand-written pages).
const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: './content/docs' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    icon: z.string().optional(),
    /** Show the description under the title (false when it is the page's own first sentence). */
    lede: z.boolean().default(true),
    /** The repository file the page is made from. */
    source: z.string().optional(),
    /** The source's own H1, when the sidebar shortens it. */
    heading: z.string().optional(),
  }),
});

const meta = defineCollection({
  loader: glob({ pattern: '**/meta.json', base: './content/docs' }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
    icon: z.string().optional(),
    root: z.boolean().optional(),
    pages: z.array(z.string()).optional(),
  }),
});

export const collections = { docs, meta };
