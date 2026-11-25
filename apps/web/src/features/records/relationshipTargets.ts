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

type PathAttribute = Pick<
  Attribute,
  'code' | 'target_blueprint_code' | 'target_blueprint_codes'
> & { value_type: string };

const sameTargets = (left: PathAttribute, right: PathAttribute) => {
  const a = new Set(allowedTargetBlueprints(left));
  const b = allowedTargetBlueprints(right);
  return a.size === new Set(b).size && b.every((code) => a.has(code));
};

/**
 * The fields of the blueprints a relationship path reaches, as the API
 * resolves them: each code once, from the first blueprint that declares it,
 * with the blueprints that do. A code the blueprints declare with different
 * value types, or as relationships to different blueprints, is a conflict a
 * path cannot continue through. `key` folds codes that match, such as by
 * case in the search syntax.
 */
export const frontierAttributes = <A extends PathAttribute>(
  targets: { code: string; attributes: A[] }[],
  key: (code: string) => string = (code) => code,
) => {
  const merged = new Map<string, { attribute: A; declaredOn: string[] }>();
  const conflicts = new Set<string>();
  for (const target of targets)
    for (const attribute of target.attributes) {
      const code = key(attribute.code);
      const first = merged.get(code);
      if (!first) {
        merged.set(code, { attribute, declaredOn: [target.code] });
        continue;
      }
      first.declaredOn.push(target.code);
      if (
        first.attribute.value_type !== attribute.value_type ||
        !sameTargets(first.attribute, attribute)
      )
        conflicts.add(code);
    }
  return {
    attributes: [...merged.values()].map(({ attribute }) => attribute),
    conflicts,
    /** The blueprints declaring each code that only some of them declare. */
    partial: new Map(
      [...merged.entries()]
        .filter(([, { declaredOn }]) => declaredOn.length < targets.length)
        .map(([code, { declaredOn }]) => [code, declaredOn]),
    ),
  };
};
