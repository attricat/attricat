import { describe, expect, it } from 'vitest';
import '../../i18n';
import { blueprintTemplates } from '../blueprints/blueprintEditorUtils';
import { NEW_ATTRIBUTE_DEFINITION } from '../reusable-attributes/constants';
import { definitionKinds } from './constants';
import { definitionDiagnostics } from './definitionDiagnostics';

const header = `format_version = 1
code = "product"
name = "Product"
kind = "entity"
`;
const attribute = `
[[attributes]]
code = "name"
value_type = "string"
`;

const diagnose = (text: string) =>
  definitionDiagnostics(definitionKinds.blueprint, text).map(
    ({ message, range }) => ({
      message,
      source: text.slice(range.start, range.end),
    }),
  );

describe('definitionDiagnostics', () => {
  it('accepts the built-in templates', () => {
    for (const template of blueprintTemplates)
      expect(diagnose(template.definition)).toEqual([]);
    expect(
      definitionDiagnostics(
        definitionKinds.reusableAttribute,
        NEW_ATTRIBUTE_DEFINITION,
      ),
    ).toEqual([]);
  });

  it('reports TOML syntax errors on their line', () => {
    const [diagnostic] = diagnose(`${header}name = \n`);
    expect(diagnostic.message).toMatch(/^Invalid TOML:/);
  });

  it('flags unknown keys and suggests close matches', () => {
    expect(diagnose(`${header}${attribute}valu_type = "string"\n`)).toEqual([
      {
        message: 'Unknown key “valu_type”. Did you mean “value_type”?',
        source: 'valu_type',
      },
    ]);
    expect(diagnose(`${header}colour = "red"\n${attribute}`)).toEqual([
      { message: 'Unknown key “colour”.', source: 'colour' },
    ]);
  });

  it('flags invalid values on the value key', () => {
    expect(diagnose(header.replace('"entity"', '"thing"') + attribute)).toEqual(
      [
        {
          message: 'Expected one of: "entity", "mixin".',
          source: 'kind',
        },
      ],
    );
    expect(
      diagnose(`${header}${attribute.replace('"string"', '"text"')}`),
    ).toEqual([expect.objectContaining({ source: 'value_type' })]);
    expect(
      diagnose(`${header.replace('"product"', '"my product"')}${attribute}`),
    ).toEqual([
      {
        message: 'Use only ASCII letters, numbers, hyphens, and underscores.',
        source: 'code',
      },
    ]);
  });

  it('reports only the selected block variant', () => {
    expect(
      diagnose(
        `${header}[views.detail]\ntype = "stack"\nchildren = [{ type = "field", field = "name", label = "Name" }]\n${attribute}`,
      ),
    ).toEqual([{ message: 'Unknown key “label”.', source: 'label' }]);
    expect(
      diagnose(
        `${header}[views.detail]\ntype = "stak"\nchildren = []\n${attribute}`,
      ),
    ).toEqual([
      expect.objectContaining({
        message: expect.stringContaining('"stack"'),
        source: 'type',
      }),
    ]);
  });

  it('points missing keys at the owning table', () => {
    expect(
      diagnose(`${header}\n[[attributes]]\nvalue_type = "string"\n`),
    ).toEqual([
      {
        message: 'Missing required key “code”.',
        source: '[[attributes]]',
      },
    ]);
  });
});
