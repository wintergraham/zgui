// What the docs' plain Markdown says by convention, shown as such: a
// paragraph opening "**Note:**" (or Correction, Warning, Tip…) and a
// blockquote become callouts, a reference page's Gotchas become a warning,
// and section headings get a link to themselves.
import { visit } from 'unist-util-visit';
import { toString } from 'hast-util-to-string';

const h = (tagName, properties, children = []) => ({ type: 'element', tagName, properties, children });
const kinds = { note: 'note', tip: 'note', correction: 'warning', warning: 'warning', caution: 'warning', important: 'warning' };
const icons = {
  note: 'M12 8h.01M11 12h1v4h1M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z',
  warning: 'M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z',
};
const icon = (kind) =>
  h('svg', { className: ['callout-icon'], viewBox: '0 0 24 24', width: 16, height: 16, ariaHidden: 'true' }, [
    h('path', { d: icons[kind], fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, strokeLinecap: 'round', strokeLinejoin: 'round' }),
  ]);
const callout = (kind, children) => h('div', { className: ['callout'], dataKind: kind }, [icon(kind), h('div', { className: ['callout-body'] }, children)]);

export default function rehypeProse() {
  return (tree) => {
    visit(tree, 'element', (node, index, parent) => {
      if (!parent || index === undefined) return;
      // **Note:** …
      if (node.tagName === 'p') {
        const first = node.children[0];
        const label = first?.type === 'element' && first.tagName === 'strong' ? toString(first).trim() : '';
        const kind = kinds[label.replace(/:$/, '').toLowerCase()];
        if (kind && label.endsWith(':')) {
          parent.children[index] = callout(kind, [node]);
          return 'skip';
        }
      }
      if (node.tagName === 'blockquote') {
        parent.children[index] = callout('note', node.children.filter((c) => c.type === 'element'));
        return 'skip';
      }
      // The list under a "Gotchas" heading.
      if (node.tagName === 'h2' && /^gotchas$/i.test(toString(node).trim())) {
        let next = index + 1;
        while (parent.children[next]?.type === 'text') next++;
        const list = parent.children[next];
        if (list?.type === 'element' && (list.tagName === 'ul' || list.tagName === 'ol')) parent.children[next] = callout('warning', [list]);
      }
      // A link to the heading itself, shown on hover.
      if ((node.tagName === 'h2' || node.tagName === 'h3') && node.properties?.id) {
        node.children.push(
          h('a', { className: ['heading-anchor'], href: `#${node.properties.id}`, ariaLabel: `Link to ${toString(node).trim()}` }, [{ type: 'text', value: '#' }]),
        );
      }
    });
  };
}
