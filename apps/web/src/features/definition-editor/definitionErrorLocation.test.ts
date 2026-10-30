import { describe, expect, it } from 'vitest';
import { locateDefinitionError } from './definitionErrorLocation';

const definition = `format_version: 1
code: product
name: Product
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
  detail:
    type: stack
    children:
      - {type: field, field: title}
      - {type: field, field: sku}
attributes:
  - code: title
    value_type: string
  - code: title
    value_type: number
`;

const located = (message: string, text = definition) => {
  const range = locateDefinitionError(text, message);
  return range && text.slice(range.start, range.end);
};
const lineOf = (message: string) => {
  const range = locateDefinitionError(definition, message);
  return range && definition.slice(0, range.start).split('\n').length;
};

describe('locateDefinitionError', () => {
  it('points at the value a quoted name refers to, inside its scope', () => {
    expect(located("view 'detail' references unknown attribute 'sku'")).toBe(
      'field',
    );
    expect(lineOf("view 'detail' references unknown attribute 'sku'")).toBe(13);
  });

  it('prefers the last occurrence, where a duplicate appears', () => {
    expect(lineOf("attribute code 'title' is duplicated")).toBe(17);
  });

  it('falls back to a key with that name', () => {
    expect(
      located("views.explorer browse_by 'shelf' must be false", 'code: a'),
    ).toBeUndefined();
    expect(located("view 'detail' must not be empty")).toBe('detail');
  });

  it('points at a key that starts the message', () => {
    expect(located('code is immutable')).toBe('code');
    expect(located('views.dropdown_option must define a field')).toBe(
      'dropdown_option',
    );
  });

  it('leaves messages it cannot place unlocated', () => {
    expect(
      located('blueprints must define at least one attribute'),
    ).toBeUndefined();
  });
});
