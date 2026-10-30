import { describe, expect, it } from 'vitest';
import { parseStrictYaml } from '../../components/yamlSyntax';
import '../../i18n';
import { definitionKinds, type DefinitionKind } from './constants';
import { definitionDiagnostics } from './definitionDiagnostics';
import {
  definitionSnippets,
  snippetEdit,
  snippetIsPresent,
  type DefinitionSnippet,
} from './definitionSnippets';
import { scanYaml } from '../../components/yamlOutline';

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

const ruleHeader = `format_version: 1
code: title-required
name: Title required
severity: error

triggers:
  - type: manual
`;

const bases: Record<DefinitionKind, string> = {
  [definitionKinds.blueprint]: `format_version: 1
code: product
name: Product
kind: record

views:
  dropdown_option:
    type: dropdown_option
    fields: [code]

attributes:
  - code: code
    value_type: string
`,
  [definitionKinds.reusableAttribute]: `code: size
name: Size
value_type: string
`,
  [definitionKinds.rule]: `${ruleHeader}
predicate:
  type: required
  attribute_code: title
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
        expect(parseStrictYaml(text)).not.toHaveProperty('error');
        expect(definitionDiagnostics(kind, text)).toEqual([]);
        expect(snippetIsPresent(scanYaml(text), snippet)).toBe(
          Boolean(snippet.conflicts),
        );
      });
    }
  }

  it('puts keys after the root keys and blocks into their parent', () => {
    const key = definitionSnippets.blueprint.find(
      ({ id }) => id === 'requiredAttributes',
    )!;
    const text = apply(bases.blueprint, key);
    expect(text).toContain('kind: record\nrecord_schema:');
    const view = definitionSnippets.blueprint.find(
      ({ id }) => id === 'tableView',
    )!;
    expect(apply(bases.blueprint, view)).toContain(
      'fields: [code]\n  table:\n    type: table\n    columns:\n      - { field: code }\n\nattributes:',
    );
    const attribute = definitionSnippets.blueprint.find(
      ({ id }) => id === 'textAttribute',
    )!;
    expect(apply(bases.blueprint, attribute)).toMatch(
      /value_type: string\n {2}- code: code\n {4}name: Name\n {4}value_type: string\n$/,
    );
  });

  it('creates a missing parent at the end, after a blank line', () => {
    const snippet = definitionSnippets.blueprint.find(
      ({ id }) => id === 'uniqueKey',
    )!;
    expect(apply(bases.blueprint, snippet)).toMatch(
      /value_type: string\n\nunique_keys:\n {2}- code: code\n {4}attributes: \[code\]\n$/,
    );
    const transition = definitionSnippets.rule.find(
      ({ id }) => id === 'enforceTransition',
    )!;
    expect(
      apply(`${bases.rule}\nenforcement:\n  on_save: true\n`, transition),
    ).toMatch(
      /enforcement:\n {2}on_save: true\n {2}transitions:\n {4}- attribute_code: status\n {6}to: released\n$/,
    );
  });

  it('appends keys to a definition with only keys', () => {
    const snippet = definitionSnippets.reusableAttribute.find(
      ({ id }) => id === 'defaultValue',
    )!;
    expect(apply('code: size', snippet)).toBe(
      'code: size\ndefault_value: value',
    );
  });

  it('is disabled when any key it adds already exists', () => {
    const target = definitionSnippets.reusableAttribute.find(
      ({ id }) => id === 'relationshipTarget',
    )!;
    expect(snippetIsPresent(scanYaml(bases.reusableAttribute), target)).toBe(
      false,
    );
    expect(
      snippetIsPresent(
        scanYaml(`${bases.reusableAttribute}cardinality: one\n`),
        target,
      ),
    ).toBe(true);
  });
});
