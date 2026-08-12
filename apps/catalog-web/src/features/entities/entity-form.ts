import type { Attribute, NewAttributeValue, RelationshipTargets } from './api';

export const valuesForForm = (
  attributes: Attribute[],
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
      const value = scalar?.value;
      return [
        attribute.code,
        typeof value === 'object' && value !== null
          ? `${value.time} ${value.time_zone}`
          : value === undefined
            ? ''
            : String(value),
      ];
    }),
  );
};

export const serializeAttributeValues = (
  attributes: Attribute[],
  fields: Record<string, string>,
): NewAttributeValue[] => {
  return attributes.flatMap<NewAttributeValue>(
    (attribute): NewAttributeValue[] => {
      const value = fields[attribute.code]?.trim();
      if (!value) return [];
      if (attribute.value_type === 'relationship') return [];
      if (attribute.value_type === 'boolean') {
        return [
          {
            kind: 'scalar' as const,
            attribute_code: attribute.code,
            value: value === 'true',
          },
        ];
      }
      if (attribute.value_type === 'number') {
        const number = Number(value);
        return Number.isFinite(number)
          ? [{ kind: 'scalar' as const, attribute_code: attribute.code, value: number }]
          : [];
      }
      if (attribute.value_type === 'integer') {
        const number = Number(value);
        return Number.isSafeInteger(number)
          ? [{ kind: 'scalar' as const, attribute_code: attribute.code, value: number }]
          : [];
      }
      if (attribute.value_type === 'time') {
        const [time, time_zone] = value.split(/\s+/, 2);
        return time && time_zone
          ? [
              {
                kind: 'scalar' as const,
                attribute_code: attribute.code,
                value: { time, time_zone },
              },
            ]
          : [];
      }
      return [{ kind: 'scalar' as const, attribute_code: attribute.code, value }];
    },
  );
};

export const relationshipTargetsForForm = (
  attributes: Attribute[],
  fields: Record<string, string>,
): RelationshipTargets[] => {
  return attributes
    .filter((attribute) => attribute.value_type === 'relationship')
    .map((attribute) => ({
      attribute_code: attribute.code,
      target_entity_ids: (fields[attribute.code] ?? '')
        .split(',')
        .map((targetEntityId) => targetEntityId.trim())
        .filter(Boolean),
    }));
};
