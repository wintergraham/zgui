import { type ReactNode, lazy, useMemo } from 'react';
import type { SerializedPageTree } from 'fumadocs-core/source/client';
import { useFumadocsLoader } from 'fumadocs-core/source/client';
import type { TOCItemType } from 'fumadocs-core/toc';
import { RootProvider } from 'fumadocs-ui/provider/astro';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import {
  DocsDescription,
  DocsPage,
  DocsTitle,
  EditOnGitHub,
  MarkdownCopyButton,
  ViewOptionsPopover,
} from 'fumadocs-ui/layouts/docs/page';
// Search's dialog and index load when it first opens, not with the page.
const Search = lazy(() => import('./search'));
import { Logo } from './logo';
import { repoUrl } from '@/lib/shared';

export interface DocsProps {
  tree: SerializedPageTree;
  pathname: string;
  params: Record<string, string | undefined>;
  title: string;
  /** The doc set's name, above the title. */
  section?: string;
  description?: string;
  toc: TOCItemType[];
  markdownUrl: string;
  /** The page's source on GitHub. */
  sourceUrl?: string;
  children?: ReactNode;
}

/**
 * The React island of a docs page: Fumadocs' layout, sidebar with the doc
 * set switcher, table of contents and search. The page's content is
 * rendered by Astro and passed in as static HTML.
 */
export function Docs({ tree, pathname, params, title, section, description, toc, markdownUrl, sourceUrl, children }: DocsProps) {
  // A copy: the loader deserializes in place, and Astro serializes these
  // props for the browser after the server has rendered them.
  const copy = useMemo(() => structuredClone(tree), [tree]);
  const data = useFumadocsLoader({ tree: copy });
  return (
    <RootProvider pathname={pathname} params={params} search={{ SearchDialog: Search }}>
      <DocsLayout
        tree={data.tree}
        nav={{ title: <Logo />, url: '/' }}
        githubUrl={repoUrl}
        tabMode="auto"
      >
        <DocsPage toc={toc} tableOfContent={{ style: 'clerk' }}>
          {section ? <p className="doc-eyebrow">{section}</p> : null}
          <DocsTitle>{title}</DocsTitle>
          {description ? <DocsDescription className="mb-0">{description}</DocsDescription> : null}
          <div className="flex flex-row flex-wrap gap-2 items-center border-b pb-6">
            <MarkdownCopyButton markdownUrl={markdownUrl} />
            <ViewOptionsPopover markdownUrl={markdownUrl} githubUrl={sourceUrl} />
          </div>
          {children}
          {sourceUrl ? <EditOnGitHub href={sourceUrl.replace('/blob/', '/edit/')} /> : null}
        </DocsPage>
      </DocsLayout>
    </RootProvider>
  );
}
