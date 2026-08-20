import { z } from 'zod';
import type {
  Attribute,
  JsonSchema,
  NewAttributeValue,
  RelationshipTargets,
} from './api';
import { scalarValueForField, valueForField } from './attribute-values';
import { jsonSchemaValidationMessage } from './json-schema';
import { attributeValueKinds, attributeValueTypes } from './value-types';

export type EntityFormValidation = {
  fieldErrors: Record<string, string>;
  formError?: string;
};

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

export const validateEntityForm = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
  requiredAttributes: readonly string[] = [],
  entitySchema?: JsonSchema | null,
): EntityFormValidation => {
  const fieldErrors: Record<string, string> = {};
  const document: Record<string, unknown> = {};

  for (const attribute of attributes) {
    const value = fields[attribute.code] ?? '';
    const required = requiredAttributes.includes(attribute.code);
    if (attribute.value_type === attributeValueTypes.relationship) {
      const targetEntityIds = relationshipIdsForField(value);
      if (required && targetEntityIds.length === 0) {
        fieldErrors[attribute.code] =
          'A value is required for the target schema.';
      } else if (!relationshipIdsAreValid(targetEntityIds)) {
        fieldErrors[attribute.code] = 'Enter comma-separated entity UUIDs.';
      } else if (targetEntityIds.length > 0) {
        document[attribute.code] = targetEntityIds;
      }
      continue;
    }

    if (required && !value.trim()) {
      fieldErrors[attribute.code] =
        'A value is required for the target schema.';
      continue;
    }
    const scalar = scalarValueForField(attribute, value);
    if (value.trim() && !scalar) {
      fieldErrors[attribute.code] =
        "Enter a value that meets this field's requirements.";
    } else if (scalar) {
      document[attribute.code] = scalar.value;
    }
  }

  if (Object.keys(fieldErrors).length > 0) return { fieldErrors };
  const formError = jsonSchemaValidationMessage(document, entitySchema);
  return formError ? { fieldErrors, formError } : { fieldErrors };
};

const relationshipIdsForField = (value: string): string[] =>
  value
    .split(',')
    .map((targetEntityId) => targetEntityId.trim())
    .filter(Boolean);

const relationshipIdsAreValid = (ids: string[]): boolean =>
  z.array(z.uuid()).safeParse(ids).success;

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
      const targetEntityIds = relationshipIdsForField(
        fields[attribute.code] ?? '',
      );
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
