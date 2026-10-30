import { describe, expect, it } from 'vitest';
import '../../i18n';
import { definitionHover } from './definitionHover';

const blueprint = `code: car
kind: record
attributes:
  - code: state
    value_type: string
    value_schema: |-
      {"type": "string", "enum": ["open", "closed"],
        "x-attricat-status": {"version": 1,
          "options": [{"code": "open", "label": "Open"}],
          "transitions": [{"from": "open", "to": "closed", "roles": ["reviewer"],
            "conditions": [{"code": "dated",
              "predicate": {"type": "compare", "attribute_code": "valid_until", "op": "gte"}}]}]}}
views:
  detail:
    type: stack
record_schema: |-
  {"required": ["state"],
    "x-attricat-checks": [{"code": "titled", "message": "Needs a title",
      "predicate": {"type": "required", "attribute_code": "state"}}]}
`;

const hover = (text: string, index = 0) => {
  let offset = -1;
  for (let count = 0; count <= index; count++)
    offset = blueprint.indexOf(text, offset + 1);
  return definitionHover('blueprint', blueprint, offset + 1)?.contents.join(
    '\n',
  );
};

describe('definitionHover', () => {
  it('explains YAML keys and the chosen variant of a tagged value', () => {
    expect(hover('value_type')).toMatch(/^\*\*value_type\*\*/);
    expect(hover('stack')).toMatch(/^\*\*stack\*\* — \S/);
  });

  it('leaves free text values unrepeated', () => {
    expect(hover('car')).toMatch(/^\*\*code\*\*/);
  });

  it('explains JSON Schema keywords in embedded schemas', () => {
    expect(hover('"required"')).toContain(
      'At a record schema’s root, attribute codes'.replace('’', "'"),
    );
    expect(hover('"enum"')).toContain('The only values allowed.');
  });

  it('explains record checks and their predicates', () => {
    expect(hover('"message"')).toContain('Shown when the check fails');
    expect(hover('"required"', 1)).toMatch(
      /^\*\*required\*\* — The attribute has a value/,
    );
  });

  it('explains status control and its transition conditions', () => {
    expect(hover('"roles"')).toContain('Roles, held through');
    expect(hover('"compare"')).toMatch(/^\*\*compare\*\* — /);
    expect(hover('"op"')).toContain('Comparison operator');
    expect(hover('"x-attricat-status"')).toContain(
      'Makes the attribute a status',
    );
  });
});
