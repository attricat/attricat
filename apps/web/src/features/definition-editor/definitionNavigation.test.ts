import { describe, expect, it } from 'vitest';
import {
  attributeDefinition,
  definitionSectionAt,
  viewDefinition,
} from './definitionNavigation';

const definition = `code: car
attributes:
  - code: vin
    value_type: string
  - code: make
    from: base.make
views:
  detail:
    type: stack
    children:
      - {type: field, field: vin}
`;

const at = (text: string) => ({
  end: definition.indexOf(text) + text.length,
  start: definition.indexOf(text),
});

describe('attributeDefinition', () => {
  it('selects the code inside the attribute it declares', () => {
    const target = attributeDefinition(definition, 'make');
    expect(target?.focus).toEqual(at('make'));
    expect(definition.slice(target?.block.start, target?.block.end)).toMatch(
      /^code: make\n\s+from: base\.make/,
    );
  });

  it('ignores the code as used elsewhere', () => {
    expect(attributeDefinition(definition, 'car')).toBeUndefined();
    expect(attributeDefinition(definition, 'missing')).toBeUndefined();
  });
});

describe('viewDefinition', () => {
  it('selects the view name and covers its body', () => {
    const target = viewDefinition(definition, 'detail');
    expect(target?.focus).toEqual(at('detail'));
    expect(definition.slice(target?.block.start, target?.block.end)).toMatch(
      /^detail:\n[\s\S]*field: vin\}/,
    );
  });

  it('finds nothing for an undeclared view', () => {
    expect(viewDefinition(definition, 'table')).toBeUndefined();
  });
});

describe('definitionSectionAt', () => {
  const offset = (text: string) => definition.indexOf(text);

  it('names the attribute whose block holds the cursor', () => {
    expect(definitionSectionAt(definition, offset('from: base'))).toEqual({
      code: 'make',
      kind: 'attribute',
    });
  });

  it('names the view and the field reference under the cursor', () => {
    expect(definitionSectionAt(definition, offset('type: stack'))).toEqual({
      field: undefined,
      kind: 'view',
      name: 'detail',
    });
    expect(definitionSectionAt(definition, offset('vin}'))).toEqual({
      field: 'vin',
      kind: 'view',
      name: 'detail',
    });
  });

  it('names nothing outside attributes and views', () => {
    expect(definitionSectionAt(definition, offset('car'))).toBeUndefined();
  });
});
