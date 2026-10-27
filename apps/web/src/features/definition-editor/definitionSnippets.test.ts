import { describe, expect, it } from 'vitest';
import { parse } from 'smol-toml';
import '../../i18n';
import { definitionKinds, type DefinitionKind } from './constants';
import { definitionDiagnostics } from './definitionDiagnostics';
import {
  definitionSnippets,
  snippetEdit,
  snippetIsPresent,
  type DefinitionSnippet,
} from './definitionSnippets';
import { scanToml } from '../../components/tomlOutline';

/** Expands Monaco snippet syntax the way accepting every default would. */
const expand = (body: string) => {
  const defaults = new Map<string, string>();
  return body
    .replace(/\$\{(\d+):([^}]*)\}/g, (_, index: string, value: string) => {
      defaults.set(index, value);
      return value;
    })
    .replace(/\$\{(\d+)\|([^|,]*)[^|]*\|\}/g, (_, index: string, value) => {
      defaults.set(index, value);
      return value;
    })
    .replace(/\$\{(\d+)\}/g, (_, index: string) => defaults.get(index) ?? 'x')
    .replaceAll('\\$', '$');
};

const apply = (text: string, snippet: DefinitionSnippet) => {
  const edit = snippetEdit(text, snippet);
  return (
    text.slice(0, edit.offset) +
    expand(edit.insertText) +
    text.slice(edit.replaceEnd ?? edit.offset)
  );
};

const ruleHeader = `format_version = 1
code = "title-required"
name = "Title required"
severity = "error"

[[triggers]]
type = "manual"
`;

const bases: Record<DefinitionKind, string> = {
  [definitionKinds.blueprint]: `format_version = 1
code = "product"
name = "Product"
kind = "record"

[views.dropdown_option]
type = "dropdown_option"
fields = ["code"]

[[attributes]]
code = "code"
value_type = "string"
`,
  [definitionKinds.reusableAttribute]: `code = "size"
name = "Size"
value_type = "string"
`,
  [definitionKinds.rule]: `${ruleHeader}
[predicate]
type = "required"
attribute_code = "title"
`,
  [definitionKinds.workflow]: '',
};

/** A rule has one predicate, so predicate snippets start without one. */
const baseFor = (kind: DefinitionKind, snippet: DefinitionSnippet) =>
  kind === definitionKinds.rule && snippet.group === 'predicates'
    ? ruleHeader
    : bases[kind];

describe('definition snippets', () => {
  for (const kind of Object.values(definitionKinds)) {
    for (const snippet of definitionSnippets[kind]) {
      it(`${kind} ${snippet.id} keeps the definition valid`, () => {
        const text = apply(baseFor(kind, snippet), snippet);
        expect(() => parse(text)).not.toThrow();
        expect(definitionDiagnostics(kind, text)).toEqual([]);
        expect(snippetIsPresent(scanToml(text), snippet)).toBe(
          Boolean(snippet.conflicts),
        );
      });
    }
  }

  it('puts keys before the first table and sections at the end', () => {
    const key = definitionSnippets.blueprint.find(
      ({ id }) => id === 'requiredAttributes',
    )!;
    const text = apply(bases.blueprint, key);
    expect(text.indexOf('record_schema')).toBeLessThan(
      text.indexOf('[views.dropdown_option]'),
    );
    expect(text).toContain('kind = "record"\n\nrecord_schema');
    const section = definitionSnippets.blueprint.find(
      ({ id }) => id === 'tableView',
    )!;
    expect(apply(bases.blueprint, section)).toMatch(
      /value_type = "string"\n\n\[views\.table\]\ntype = "table"\ncolumns = \[\{ field = "code" \}\]\n$/,
    );
  });

  it('appends keys to a definition without tables', () => {
    const snippet = definitionSnippets.reusableAttribute.find(
      ({ id }) => id === 'defaultValue',
    )!;
    expect(apply('code = "size"', snippet)).toBe(
      'code = "size"\ndefault_value = "value"\n',
    );
  });

  it('is disabled when any key it adds already exists', () => {
    const target = definitionSnippets.reusableAttribute.find(
      ({ id }) => id === 'relationshipTarget',
    )!;
    expect(snippetIsPresent(scanToml(bases.reusableAttribute), target)).toBe(
      false,
    );
    expect(
      snippetIsPresent(
        scanToml(`${bases.reusableAttribute}cardinality = "one"\n`),
        target,
      ),
    ).toBe(true);
  });
});
