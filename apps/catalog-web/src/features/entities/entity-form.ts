import { z } from 'zod';
import type { Attribute, NewAttributeValue, RelationshipTargets } from './api';
import { scalarValueForField, valueForField } from './attribute-values';
import { attributeValueKinds, attributeValueTypes } from './value-types';

export const valuesForForm = (
  attributes: readonly Attribute[],
  values: NewAttributeValue[],
  contextId: string | null = null,
): Record<string, string> => {
  return Object.fromEntries(
    attributes.map((attribute) => {
      const matching = values.filter(
        (value) =>
          value.attribute_code === attribute.code &&
          (value.context_id ?? null) === contextId,
      );
      if (attribute.value_type === attributeValueTypes.relationship) {
        return [
          attribute.code,
          matching
            .filter(
              (
                value,
              ): value is Extract<
                NewAttributeValue,
                { kind: typeof attributeValueKinds.relationship }
              > => value.kind === attributeValueKinds.relationship,
            )
            .map((value) => value.target_entity_id)
            .join(', '),
        ];
      }
      const scalar = matching.find(
        (
          value,
        ): value is Extract<
          NewAttributeValue,
          { kind: typeof attributeValueKinds.scalar }
        > => value.kind === attributeValueKinds.scalar,
      );
      return [attribute.code, valueForField(scalar?.value)];
    }),
  );
};

export const serializeAttributeValues = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
  contextId: string | null = null,
): NewAttributeValue[] => {
  return attributes.flatMap<NewAttributeValue>(
    (attribute): NewAttributeValue[] => {
      if (attribute.value_type === attributeValueTypes.relationship) return [];
      const scalar = scalarValueForField(
        attribute,
        fields[attribute.code] ?? '',
      );
      return scalar ? [{ ...scalar, context_id: contextId }] : [];
    },
  );
};

export const hasInvalidScalarField = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
): boolean =>
  attributes.some(
    (attribute) =>
      attribute.value_type !== attributeValueTypes.relationship &&
      Boolean(fields[attribute.code]?.trim()) &&
      !scalarValueForField(attribute, fields[attribute.code]),
  );

export const relationshipTargetsForForm = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
  contextId: string | null = null,
): RelationshipTargets[] => {
  return attributes
    .filter(
      (attribute) => attribute.value_type === attributeValueTypes.relationship,
    )
    .flatMap((attribute) => {
      const targetEntityIds = (fields[attribute.code] ?? '')
        .split(',')
        .map((targetEntityId) => targetEntityId.trim())
        .filter(Boolean);
      const result = z.array(z.uuid()).safeParse(targetEntityIds);
      return result.success
        ? [
            {
              attribute_code: attribute.code,
              context_id: contextId,
              target_entity_ids: result.data,
            },
          ]
        : [];
    });
};
