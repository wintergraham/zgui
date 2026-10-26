// The frame every code block on the site gets, at build time: a header with
// the file's name or the language and a copy button, the lines laid out as a
// grid (so highlights and diff marks span the block), and adjacent blocks
// that each name a file joined as tabs. Its behaviour (copy, tabs) is
// components/CodeBlocks.astro's; its look is global.css's `.cb`.
import { visit } from 'unist-util-visit';

const names = {
  rust: 'Rust', luau: 'Luau', lua: 'Luau', sh: 'Shell', bash: 'Shell', shell: 'Shell', console: 'Shell',
  json: 'JSON', jsonc: 'JSON', toml: 'TOML', powershell: 'PowerShell', text: 'Text', wgsl: 'WGSL',
  swift: 'Swift', diff: 'Diff', yaml: 'YAML', python: 'Python', ts: 'TypeScript', js: 'JavaScript',
};

/** An element's classes, whether it was made with `className` or (as Shiki's are) `class`. */
const classes = (n) => {
  const c = n?.properties?.className ?? n?.properties?.class ?? [];
  return (Array.isArray(c) ? c : String(c).split(/\s+/)).map(String);
};

const h = (tagName, properties, children = []) => ({ type: 'element', tagName, properties, children });
const text = (value) => ({ type: 'text', value });

const copyIcon = h('svg', { viewBox: '0 0 24 24', width: 14, height: 14, ariaHidden: 'true' }, [
  h('rect', { x: 9, y: 9, width: 12, height: 12, rx: 2.5, fill: 'none', stroke: 'currentColor', strokeWidth: 1.8 }),
  h('path', { d: 'M15 9V5.5A2.5 2.5 0 0 0 12.5 3h-7A2.5 2.5 0 0 0 3 5.5v7A2.5 2.5 0 0 0 5.5 15H9', fill: 'none', stroke: 'currentColor', strokeWidth: 1.8 }),
]);
const copyButton = () => h('button', { type: 'button', className: ['cb-copy'], ariaLabel: 'Copy code', title: 'Copy' }, [copyIcon]);

/** The language of a highlighted `pre`, from its code's `language-*` class. */
function langOf(pre) {
  const code = pre.children.find((c) => c.type === 'element' && c.tagName === 'code');
  const cls = [...classes(code), ...classes(pre)].find((c) => c.startsWith('language-'));
  return cls ? String(cls).slice(9) : pre.properties?.dataLanguage ?? 'text';
}

function prepare(pre) {
  pre.data = { ...pre.data, framed: true };
  const lang = langOf(pre);
  const title = pre.properties.title ?? pre.properties.dataTitle;
  delete pre.properties.title;
  delete pre.properties.dataTitle;
  delete pre.properties.icon;
  pre.properties.className = ['cb-pre', ...classes(pre)];
  delete pre.properties.class;
  delete pre.properties.tabindex;
  pre.properties.tabIndex = 0;
  // Shiki's newlines between lines would be blank rows in a grid.
  const code = pre.children.find((c) => c.type === 'element' && c.tagName === 'code');
  if (code) code.children = code.children.filter((c) => !(c.type === 'text' && /^\s*$/.test(c.value)));
  return { lang, title: title ? String(title) : undefined, label: names[lang] ?? lang };
}

/** One block: its header (a file's name, or the language) and its code. */
export function frame(pre) {
  const { lang, title, label } = prepare(pre);
  const head = [
    title ? h('span', { className: ['cb-title'] }, [text(title)]) : h('span', { className: ['cb-lang'] }, [text(label)]),
    ...(title ? [h('span', { className: ['cb-lang', 'cb-lang-end'] }, [text(label)])] : []),
    copyButton(),
  ];
  return h('figure', { className: ['cb', 'not-fumadocs-codeblock'], dataLang: lang }, [h('div', { className: ['cb-head'] }, head), pre]);
}

/** Blocks shown one at a time under tabs named by their files. */
export function tabs(pres, id) {
  const items = pres.map((pre) => ({ pre, ...prepare(pre) }));
  const tablist = h(
    'div',
    { className: ['cb-tabs'], role: 'tablist', ariaLabel: 'Files' },
    items.map((item, i) =>
      h('button', {
        type: 'button', role: 'tab', id: `${id}-tab-${i}`, ariaControls: `${id}-panel-${i}`,
        ariaSelected: i === 0 ? 'true' : 'false', tabIndex: i === 0 ? 0 : -1, className: ['cb-tab'], dataLang: item.lang,
      }, [text(item.title ?? item.label)]),
    ),
  );
  const panels = items.map((item, i) =>
    h('div', { role: 'tabpanel', id: `${id}-panel-${i}`, ariaLabelledby: `${id}-tab-${i}`, className: ['cb-panel'], dataLang: item.lang, hidden: i !== 0 }, [item.pre]),
  );
  return h('div', { className: ['cb', 'cb-group', 'not-fumadocs-codeblock'] }, [h('div', { className: ['cb-head'] }, [tablist, copyButton()]), ...panels]);
}

const isPre = (n) => n?.type === 'element' && n.tagName === 'pre' && !n.data?.framed && classes(n).includes('shiki');
const titled = (n) => isPre(n) && (n.properties.title || n.properties.dataTitle);

/** The rehype plugin: frames every highlighted block; adjacent named ones become tabs. */
export default function rehypeCodeFrame() {
  return (tree) => {
    // Fumadocs wraps each highlighted block in a root of its own: lift them,
    // so blocks that follow each other are siblings.
    const flatten = (node) => {
      if (!node.children) return;
      node.children = node.children.flatMap((c) => (c.type === 'root' ? c.children : [c]));
      node.children.forEach(flatten);
    };
    flatten(tree);
    let group = 0;
    visit(tree, (node) => {
      if (!node.children?.some(isPre)) return;
      const out = [];
      const kids = node.children;
      for (let i = 0; i < kids.length; i++) {
        if (!isPre(kids[i])) {
          out.push(kids[i]);
          continue;
        }
        // Named blocks with only whitespace between them run together.
        const run = [kids[i]];
        let j = i + 1;
        while (titled(kids[i])) {
          let k = j;
          while (k < kids.length && kids[k].type === 'text' && /^\s*$/.test(kids[k].value)) k++;
          if (!titled(kids[k])) break;
          run.push(kids[k]);
          j = k + 1;
        }
        if (run.length > 1) {
          out.push(tabs(run, `cb${group++}`));
          i = j - 1;
        } else out.push(frame(kids[i]));
      }
      node.children = out;
    });
  };
}
