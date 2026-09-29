import { z } from 'zod';
import i18n from '../../i18n';
import type {
  Attribute,
  JsonSchema,
  NewAttributeValue,
  FormAttributeValue,
  RelationshipTargets,
} from './api';
import { scalarValueForField, valueForField } from './attributeValues';
import { jsonSchemaValidationErrors } from './jsonSchema';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';
import { RELATIONSHIP_ID_JOINER, RELATIONSHIP_ID_SEPARATOR } from './constants';

export type EntityFormValidation = {
  fieldErrors: Record<string, string>;
  formError?: string;
};

export const valuesForForm = (
  attributes: readonly Attribute[],
  values: FormAttributeValue[],
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
            .join(RELATIONSHIP_ID_JOINER),
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

export type EntityFormValidationMessages = {
  invalidRelationship: string;
  invalidValue: string;
  required: string;
  schema: string;
};

/** Resolves validation messages in the active language at validation time. */
export const entityFormValidationMessages =
  (): EntityFormValidationMessages => ({
    invalidRelationship: i18n.t('entities.invalidRelationshipValue'),
    invalidValue: i18n.t('entities.invalidAttributeValue'),
    required: i18n.t('entities.requiredAttributeValue'),
    schema: i18n.t('entities.schemaValidationFailed'),
  });

export const validateEntityForm = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
  requiredAttributes: readonly string[] = [],
  entitySchema?: JsonSchema | null,
  messages: EntityFormValidationMessages = entityFormValidationMessages(),
): EntityFormValidation => {
  const fieldErrors: Record<string, string> = {};
  const document: Record<string, unknown> = {};

  for (const attribute of attributes) {
    const value = fields[attribute.code] ?? '';
    const required = requiredAttributes.includes(attribute.code);
    if (attribute.value_type === attributeValueTypes.file) continue;
    if (attribute.value_type === attributeValueTypes.relationship) {
      const targetEntityIds = relationshipIdsForField(value);
      if (required && targetEntityIds.length === 0) {
        fieldErrors[attribute.code] = messages.required;
      } else if (!relationshipIdsAreValid(targetEntityIds)) {
        fieldErrors[attribute.code] = messages.invalidRelationship;
      } else if (targetEntityIds.length > 0) {
        document[attribute.code] = targetEntityIds;
      }
      continue;
    }

    if (required && !value.trim()) {
      fieldErrors[attribute.code] = messages.required;
      continue;
    }
    const scalar = scalarValueForField(attribute, value);
    if (value.trim() && !scalar) {
      fieldErrors[attribute.code] = messages.invalidValue;
    } else if (scalar) {
      document[attribute.code] = scalar.value;
    }
  }

  if (Object.keys(fieldErrors).length > 0) return { fieldErrors };
  const schemaErrors = jsonSchemaValidationErrors(document, entitySchema);
  if (schemaErrors === undefined)
    return {
      fieldErrors,
      formError: messages.schema,
    };
  const formError = schemaErrors
    .map((error) => {
      const attributeCode = attributeCodeForSchemaError(error);
      if (
        attributeCode &&
        attributes.some((attribute) => attribute.code === attributeCode)
      ) {
        fieldErrors[attributeCode] ??= messages.schema;
        return undefined;
      }
      return messages.schema;
    })
    .find(Boolean);
  return formError ? { fieldErrors, formError } : { fieldErrors };
};

const attributeCodeForSchemaError = (error: {
  instancePath: string;
  keyword: string;
  params: Record<string, unknown>;
}): string | undefined => {
  if (
    error.keyword === 'required' &&
    typeof error.params.missingProperty === 'string'
  )
    return error.params.missingProperty;
  const [segment] = error.instancePath.split('/').filter(Boolean);
  return segment?.replaceAll('~1', '/').replaceAll('~0', '~');
};

export const relationshipIdsForField = (value: string): string[] =>
  value
    .split(RELATIONSHIP_ID_SEPARATOR)
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
