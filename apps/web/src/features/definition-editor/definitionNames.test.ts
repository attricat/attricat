import { describe, expect, it } from 'vitest';
import { scanYaml } from '../../components/yamlOutline';
import { declaredNameAt, nameUsages, renameProblem } from './definitionNames';

const definition = `code: car
includes:
  - alias: seo
    code: seo
    version: 1
attributes:
  - code: maker
    value_type: relationship
    target_blueprint: maker
  - code: "title"
    value_type: string
  - code: meta_title
    from: seo.meta_title
views:
  dropdown_option:
    type: dropdown_option
    fields: [title, maker.title]
  table:
    type: table
    columns:
      - {field: maker.title}
`;
const outline = scanYaml(definition);
const kind = 'blueprint';
const nth = (text: string, index = 0) => {
  let offset = -1;
  for (let count = 0; count <= index; count++)
    offset = definition.indexOf(text, offset + 1);
  return offset;
};
const slices = (ranges: { end: number; start: number }[]) =>
  ranges.map(({ end, start }) => `${start}:${definition.slice(start, end)}`);

describe('declaredNameAt', () => {
  it('finds a name from its declaration or a reference to it', () => {
    expect(
      declaredNameAt(kind, definition, outline, nth('maker', 0) + 1)?.name,
    ).toBe('maker');
    expect(
      declaredNameAt(kind, definition, outline, nth('maker.title') + 1),
    ).toEqual({
      declaration: { end: nth('maker') + 'maker'.length, start: nth('maker') },
      name: 'maker',
      table: 'attributes',
    });
    expect(
      declaredNameAt(kind, definition, outline, nth('seo.meta') + 1)?.name,
    ).toBe('seo');
  });

  it('skips segments that name another blueprint’s attributes', () => {
    expect(
      declaredNameAt(kind, definition, outline, nth('maker.title') + 7),
    ).toBeUndefined();
  });

  it('declares a quoted code without its quotes', () => {
    expect(
      declaredNameAt(kind, definition, outline, nth('"title"') + 2)
        ?.declaration,
    ).toEqual({ end: nth('"title"') + 6, start: nth('"title"') + 1 });
  });
});

describe('nameUsages', () => {
  it('lists the declaration and every reference to it', () => {
    const maker = declaredNameAt(kind, definition, outline, nth('maker') + 1);
    expect(slices(nameUsages(kind, definition, outline, maker!))).toEqual([
      `${nth('maker')}:maker`,
      `${nth('maker.title')}:maker`,
      `${nth('maker.title', 1)}:maker`,
    ]);
  });
});

describe('renameProblem', () => {
  const title = declaredNameAt(
    kind,
    definition,
    outline,
    definition.indexOf('"title"') + 1,
  )!;

  it('accepts codes and rejects anything else', () => {
    expect(renameProblem(outline, title, 'new_code-2')).toBeUndefined();
    expect(renameProblem(outline, title, 'new code')).toBeDefined();
  });

  it('rejects a name another attribute or include declares', () => {
    expect(renameProblem(outline, title, 'title')).toBeUndefined();
    expect(renameProblem(outline, title, 'maker')).toBeDefined();
    expect(renameProblem(outline, title, 'seo')).toBeDefined();
  });
});
