import type { BlueprintWithAttributes } from './schemas';

const sameJson = (left: unknown, right: unknown) =>
  JSON.stringify(left) === JSON.stringify(right);

/**
 * A migration is safe to run automatically when the record schema is
 * unchanged and every retained attribute keeps its storage contract.
 */
export const isSafeAutomaticMigration = (
  source: BlueprintWithAttributes,
  target: BlueprintWithAttributes,
) => {
  if (!sameJson(source.blueprint.record_schema, target.blueprint.record_schema))
    return false;
  const targetAttributes = new Map(
    target.attributes.map((attribute) => [attribute.code, attribute]),
  );
  return source.attributes.every((attribute) => {
    const targetAttribute = targetAttributes.get(attribute.code);
    return (
      targetAttribute === undefined ||
      (attribute.value_type === targetAttribute.value_type &&
        sameJson(attribute.value_schema, targetAttribute.value_schema) &&
        sameJson(attribute.default_value, targetAttribute.default_value) &&
        sameJson(attribute.file_policy, targetAttribute.file_policy) &&
        attribute.target_blueprint_code ===
          targetAttribute.target_blueprint_code &&
        sameJson(
          attribute.target_blueprint_codes ?? [],
          targetAttribute.target_blueprint_codes ?? [],
        ) &&
        attribute.cardinality === targetAttribute.cardinality &&
        attribute.target_cardinality === targetAttribute.target_cardinality &&
        attribute.context_fallback === targetAttribute.context_fallback &&
        attribute.context_editable === targetAttribute.context_editable &&
        attribute.readonly === targetAttribute.readonly)
    );
  });
};
