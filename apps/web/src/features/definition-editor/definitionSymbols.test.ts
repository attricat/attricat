import { describe, expect, it } from 'vitest';
import { definitionSymbols, type DefinitionSymbol } from './definitionSymbols';

const definition = `code: car
attributes:
  - code: vin
    value_type: string
views:
  detail:
    type: stack
    children:
      - {type: field, field: vin}
      - type: tabs
        tabs: []
`;

type Shape = {
  children?: Shape[];
  detail?: string;
  kind: string;
  name: string;
};

const shape = ({ children, detail, kind, name }: DefinitionSymbol): Shape => ({
  ...(children.length ? { children: children.map(shape) } : {}),
  ...(detail === undefined ? {} : { detail }),
  kind,
  name,
});

describe('definitionSymbols', () => {
  it('outlines sections, named items and typed blocks', () => {
    expect(definitionSymbols(definition).map(shape)).toEqual([
      { detail: 'car', kind: 'property', name: 'code' },
      {
        children: [{ kind: 'item', name: 'vin' }],
        kind: 'array',
        name: 'attributes',
      },
      {
        children: [
          {
            children: [
              {
                children: [
                  { detail: 'field', kind: 'item', name: 'vin' },
                  {
                    children: [{ kind: 'array', name: 'tabs' }],
                    kind: 'item',
                    name: 'tabs',
                  },
                ],
                kind: 'array',
                name: 'children',
              },
            ],
            kind: 'object',
            name: 'detail',
          },
        ],
        kind: 'object',
        name: 'views',
      },
    ]);
  });

  it('spans an item from its first line to its last value', () => {
    const [, attributes] = definitionSymbols(definition);
    const [vin] = attributes.children;
    expect(definition.slice(vin.range.start, vin.range.end)).toBe(
      'code: vin\n    value_type: string\n',
    );
    expect(
      definition.slice(
        attributes.selectionRange.start,
        attributes.selectionRange.end,
      ),
    ).toBe('attributes');
  });
});
