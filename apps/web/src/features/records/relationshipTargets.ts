import type { Attribute } from './api';

/** Every blueprint a relationship may target; empty allows any blueprint. */
export const allowedTargetBlueprints = (
  attribute: Pick<
    Attribute,
    'target_blueprint_code' | 'target_blueprint_codes'
  >,
): string[] =>
  attribute.target_blueprint_codes?.length
    ? attribute.target_blueprint_codes
    : attribute.target_blueprint_code
      ? [attribute.target_blueprint_code]
      : [];

/** How [`frontierAttributes`] reads an attribute of some shape. */
export type PathFields<A> = {
  code: (attribute: A) => string;
  valueType: (attribute: A) => string;
  /** A relationship's target blueprints; none for other attributes. */
  targets: (attribute: A) => string[];
};

/** Reads API attributes for [`frontierAttributes`]. */
export const attributePathFields: PathFields<
  Pick<
    Attribute,
    'code' | 'value_type' | 'target_blueprint_code' | 'target_blueprint_codes'
  >
> = {
  code: (attribute) => attribute.code,
  valueType: (attribute) => attribute.value_type,
  targets: allowedTargetBlueprints,
};

/** Whether two lists hold the same codes, in any order. */
const sameCodes = (left: string[], right: string[]) =>
  left.length === right.length &&
  [...left].sort().join() === [...right].sort().join();

/**
 * The fields of the blueprints a relationship path reaches, as the API
 * resolves them: each code once, from the first blueprint that declares it.
 * A code the blueprints declare with different value types, or as
 * relationships to different blueprints, is a conflict a path cannot
 * continue through; `partial` names the blueprints of a code only some of
 * them declare. `key` folds codes that match, such as by case in the search
 * syntax.
 */
export const frontierAttributes = <A>(
  targets: { code: string; attributes: readonly A[] }[],
  fields: PathFields<A>,
  key: (code: string) => string = (code) => code,
) => {
  const merged = new Map<string, { attribute: A; declaredOn: string[] }>();
  const conflicts = new Set<string>();
  for (const target of targets)
    for (const attribute of target.attributes) {
      const code = key(fields.code(attribute));
      const first = merged.get(code);
      if (!first) {
        merged.set(code, { attribute, declaredOn: [target.code] });
        continue;
      }
      first.declaredOn.push(target.code);
      if (
        fields.valueType(first.attribute) !== fields.valueType(attribute) ||
        !sameCodes(fields.targets(first.attribute), fields.targets(attribute))
      )
        conflicts.add(code);
    }
  return {
    attributes: [...merged.values()].map(({ attribute }) => attribute),
    conflicts,
    partial: new Map(
      [...merged.entries()]
        .filter(([, { declaredOn }]) => declaredOn.length < targets.length)
        .map(([code, { declaredOn }]) => [code, declaredOn]),
    ),
  };
};
