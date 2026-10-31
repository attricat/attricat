import { describe, expect, it } from 'vitest';
import { scanYaml } from '../../components/yamlOutline';
import { declaredNameAt, nameUsages } from './definitionNames';
import {
  referenceAt,
  referenceDeclaration,
} from './definitionReferenceTargets';

const definition = `attributes:
  - code: valid_from
    value_type: date
  - code: valid_until
    value_type: date
  - code: supplier
    value_type: relationship
    target_blueprint: supplier
  - code: facility
    value_type: relationship
    target_blueprint: facility
  - code: state
    value_type: string
    value_schema: |-
      {"type": "string", "enum": ["open", "closed"],
        "x-attricat-status": {"version": 1,
          "options": [{"code": "open", "label": "Open"},
            {"code": "closed", "label": "Closed", "lock": ["valid_from"],
              "approval": {"covers": ["valid_from"], "void_to": "open"}}],
          "transitions": [
          {"from": "open", "to": "closed", "conditions": [
            {"code": "dated", "predicate": {"type": "required", "attribute_code": "valid_from"}}]}]}}
record_schema: |-
  {
    "required": ["valid_from"],
    "properties": {"valid_until": {}},
    "dependentRequired": {"valid_until": ["valid_from"]},
    "allOf": [{"if": {"required": ["valid_from"]}, "then": {"required": ["valid_until"]}}],
    "x-attricat-checks": [
      {"code": "range", "predicate": {"type": "compare",
        "attribute_code": "valid_until", "op": "gte", "other_attribute_code": "valid_from"}},
      {"code": "facility-of-supplier", "predicate": {"type": "linked",
        "relationship_code": "facility",
        "predicate": {"type": "compare", "attribute_code": "supplier", "op": "eq",
          "subject_attribute_code": "supplier"}}}
    ]
  }
`;
const outline = scanYaml(definition);
const kind = 'blueprint';

/** Offsets of each quoted occurrence of `code` after the attribute list. */
const quoted = (code: string) => {
  const offsets: number[] = [];
  let offset = definition.indexOf('value_schema');
  while ((offset = definition.indexOf(`"${code}"`, offset + 1)) >= 0)
    offsets.push(offset);
  return offsets;
};

const declaration = (code: string) => {
  const start = definition.indexOf(`code: ${code}`) + 'code: '.length;
  return { end: start + code.length, start };
};

describe('embedded references', () => {
  it('lead from record schema keywords and checks to the attribute', () => {
    for (const offset of quoted('valid_until')) {
      const target = referenceAt(kind, outline, offset + 1);
      expect(target && referenceDeclaration(outline, target)).toEqual(
        declaration('valid_until'),
      );
    }
  });

  it('lead from status transition conditions to the attribute', () => {
    const [condition] = quoted('valid_from');
    expect(referenceAt(kind, outline, condition + 1)?.value).toBe('valid_from');
  });

  it('treat codes inside linked as the linked record’s', () => {
    const [linkedCode, subjectCode] = quoted('supplier');
    expect(referenceAt(kind, outline, linkedCode + 1)).toBeUndefined();
    expect(referenceAt(kind, outline, subjectCode + 1)?.value).toBe('supplier');
    expect(
      referenceAt(kind, outline, quoted('facility')[0] + 1)?.reference,
    ).toBe('relationship_attribute');
  });

  it('include conditional schemas and status locks and approvals', () => {
    for (const code of ['valid_from', 'valid_until']) {
      const nested = quoted(code).filter(
        (offset) =>
          offset > definition.indexOf('"allOf"') ||
          (offset > definition.indexOf('"options"') &&
            offset < definition.indexOf('"transitions"')),
      );
      expect(nested.length).toBeGreaterThan(0);
      for (const offset of nested)
        expect(referenceAt(kind, outline, offset + 1)?.value).toBe(code);
    }
  });

  it('are renamed with the attribute', () => {
    const declared = declaredNameAt(
      kind,
      definition,
      outline,
      quoted('valid_from')[1] + 1,
    );
    expect(nameUsages(kind, definition, outline, declared!)).toHaveLength(
      1 + quoted('valid_from').length,
    );
  });

  it('skip JSON that is not valid yet without failing', () => {
    const typing = definition.replace('"op": "gte",', '"op": ');
    expect(() => referenceAt(kind, scanYaml(typing), 0)).not.toThrow();
  });
});
