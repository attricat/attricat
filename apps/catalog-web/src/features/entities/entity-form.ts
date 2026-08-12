import { z } from 'zod';
import type { Attribute, NewAttributeValue, RelationshipTargets } from './api';
import { scalarValueForField, valueForField } from './attribute-values';

export const valuesForForm = (
  attributes: readonly Attribute[],
  values: NewAttributeValue[],
): Record<string, string> => {
  return Object.fromEntries(
    attributes.map((attribute) => {
      const matching = values.filter(
        (value) => value.attribute_code === attribute.code,
      );
      if (attribute.value_type === 'relationship') {
        return [
          attribute.code,
          matching
            .filter(
              (
                value,
              ): value is Extract<
                NewAttributeValue,
                { kind: 'relationship' }
              > => value.kind === 'relationship',
            )
            .map((value) => value.target_entity_id)
            .join(', '),
        ];
      }
      const scalar = matching.find(
        (value): value is Extract<NewAttributeValue, { kind: 'scalar' }> =>
          value.kind === 'scalar',
      );
      return [attribute.code, valueForField(scalar?.value)];
    }),
  );
};

export const serializeAttributeValues = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
): NewAttributeValue[] => {
  return attributes.flatMap<NewAttributeValue>(
    (attribute): NewAttributeValue[] => {
      if (attribute.value_type === 'relationship') return [];
      const scalar = scalarValueForField(
        attribute,
        fields[attribute.code] ?? '',
      );
      return scalar ? [scalar] : [];
    },
  );
};

export const relationshipTargetsForForm = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
): RelationshipTargets[] => {
  return attributes
    .filter((attribute) => attribute.value_type === 'relationship')
    .flatMap((attribute) => {
      const targetEntityIds = (fields[attribute.code] ?? '')
        .split(',')
        .map((targetEntityId) => targetEntityId.trim())
        .filter(Boolean);
      const result = z.array(z.uuid()).safeParse(targetEntityIds);
      return result.success
        ? [{ attribute_code: attribute.code, target_entity_ids: result.data }]
        : [];
    });
};
