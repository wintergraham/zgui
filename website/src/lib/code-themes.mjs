// zgx's code themes, light and dark: Geist's code palette (strings green,
// keywords rose, functions violet, constants blue) tuned to the site's green,
// every token at 4.5:1 or more on its background (scripts/check-contrast.mjs).

/** @param {{ name: string, type: 'light' | 'dark', bg: string, fg: string, c: Record<string, string> }} t */
function theme({ name, type, bg, fg, c }) {
  const rule = (scope, foreground, fontStyle) => ({ scope, settings: { foreground, ...(fontStyle ? { fontStyle } : {}) } });
  return {
    name,
    type,
    colors: { 'editor.background': bg, 'editor.foreground': fg },
    fg,
    bg,
    tokenColors: [
      rule(['source', 'text', 'variable', 'variable.other', 'meta.definition.variable', 'variable.parameter'], fg),
      rule(['comment', 'punctuation.definition.comment', 'string.comment'], c.comment),
      rule(['punctuation', 'meta.brace', 'keyword.operator', 'punctuation.separator', 'punctuation.terminator', 'punctuation.accessor', 'meta.delimiter'], c.punct),
      rule(['keyword', 'storage', 'storage.type', 'storage.modifier', 'keyword.control', 'keyword.other', 'keyword.declaration', 'variable.language', 'keyword.operator.new', 'keyword.operator.logical.lua', 'keyword.operator.lua'], c.keyword),
      rule(['string', 'string.quoted', 'punctuation.definition.string', 'string.template', 'string.other', 'markup.inline.raw', 'string.unquoted', 'constant.character.escape'], c.string),
      rule(['meta.template.expression', 'meta.interpolation', 'punctuation.definition.template-expression', 'punctuation.section.interpolation'], c.constant),
      rule(['constant', 'constant.numeric', 'constant.language', 'constant.character', 'constant.other', 'support.constant', 'variable.other.constant', 'variable.other.enummember', 'constant.other.option'], c.constant),
      rule(['entity.name.function', 'support.function', 'variable.function', 'meta.function-call entity.name.function', 'entity.name.function.macro', 'entity.name.command', 'support.function.builtin'], c.func),
      rule(['entity.name.type', 'entity.name.class', 'entity.name.struct', 'entity.name.enum', 'entity.name.trait', 'support.type', 'support.class', 'entity.name.type.primitive', 'entity.name.namespace', 'entity.other.inherited-class', 'storage.type.primitive'], c.type),
      rule(['support.type.property-name', 'support.type.property-name.json', 'entity.name.tag', 'meta.object-literal.key', 'entity.other.attribute-name'], c.func),
      rule(['meta.attribute', 'punctuation.definition.attribute', 'meta.annotation'], c.comment),
      rule(['markup.inserted', 'meta.diff.header.to-file'], c.string),
      rule(['markup.deleted', 'meta.diff.header.from-file'], c.keyword),
      rule(['markup.heading'], c.func, 'bold'),
    ],
  };
}

export const zgxLight = theme({
  name: 'zgx-light',
  type: 'light',
  bg: '#ffffff',
  fg: '#1a1a1a',
  c: { comment: '#6b6b6b', punct: '#5c5c5c', keyword: '#c4245c', string: '#1a7a43', constant: '#0b63c7', func: '#7a3cc4', type: '#965800' },
});

export const zgxDark = theme({
  name: 'zgx-dark',
  type: 'dark',
  bg: '#111111',
  fg: '#ececec',
  c: { comment: '#8c8c8c', punct: '#a3a3a3', keyword: '#ff7aa4', string: '#6fd391', constant: '#6cb6ff', func: '#c79dff', type: '#f0c46c' },
});

export const codeThemes = { light: zgxLight, dark: zgxDark };
