import type { TFunction } from 'i18next';
import type { Attribute, RecordMigrationPreview, ViewDefinition } from './api';
import { valueForField } from './attributeValues';
import { migrationIssueKinds } from './constants';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';

export type MigrationIssue = RecordMigrationPreview['issues'][number];
type MigrationValue = RecordMigrationPreview['values'][number];

/**
 * Splits migration issues into those shown on target fields, those about a value
 * in another context, which the form's fields can't answer, and the rest.
 */
export const partitionMigrationIssues = (
  preview: RecordMigrationPreview | undefined,
  defaultContextId: string | null | undefined,
) => {
  const targetAttributeCodes = new Set(
    preview?.target.attributes.map((attribute) => attribute.code) ?? [],
  );
  const issues = preview?.issues ?? [];
  const contextIssues = issues.filter(
    (issue) =>
      issue.attribute_code !== null &&
      targetAttributeCodes.has(issue.attribute_code) &&
      Boolean(issue.context_id) &&
      issue.context_id !== defaultContextId,
  );
  const fieldIssues = issues.filter((issue) => !contextIssues.includes(issue));
  const inlineIssues = fieldIssues.filter(
    (issue) =>
      issue.attribute_code !== null &&
      targetAttributeCodes.has(issue.attribute_code),
  );
  return {
    inlineIssues,
    contextIssues,
    standaloneIssues: fieldIssues.filter(
      (issue) => !inlineIssues.includes(issue),
    ),
  };
};

/** A stored value outside the default context that needs an answer. */
export type MigrationContextValue = {
  key: string;
  attributeCode: string;
  contextId: string;
  message: string;
  /** Only a scalar still editable in that context can be replaced. */
  replaceable: boolean;
};

/** The answer to one such value: a replacement, or removing it. */
export type MigrationContextAnswer = { remove: boolean; text: string };

/** The `data-field-code` of such a value, so an error can lead to it. */
export const contextValueFieldCode = (
  attributeCode: string,
  contextId: string,
) => `context:${attributeCode}:${contextId}`;

/** One entry per attribute and context, however many issues it has. */
export const migrationContextValues = (
  issues: readonly MigrationIssue[],
  attributes: readonly Attribute[],
  t: TFunction,
): MigrationContextValue[] => {
  const byKey = new Map<string, MigrationContextValue>();
  for (const issue of issues) {
    const attributeCode = issue.attribute_code!;
    const contextId = issue.context_id!;
    const key = contextValueFieldCode(attributeCode, contextId);
    const message = migrationIssueMessage(issue, t);
    const seen = byKey.get(key);
    const valueType = attributes.find(
      (attribute) => attribute.code === attributeCode,
    )?.value_type;
    const replaceable =
      issue.kind !== migrationIssueKinds.contextNotEditable &&
      valueType !== attributeValueTypes.relationship &&
      valueType !== attributeValueTypes.file;
    byKey.set(key, {
      key,
      attributeCode,
      contextId,
      message: seen ? `${seen.message} ${message}` : message,
      replaceable: (seen?.replaceable ?? true) && replaceable,
    });
  }
  return [...byKey.values()];
};

/** Whether the user has said what happens to a value. */
export const contextValueAnswered = (
  value: MigrationContextValue,
  answer: MigrationContextAnswer | undefined,
) =>
  Boolean(answer?.remove) ||
  (value.replaceable && Boolean(answer?.text.trim()));

/** Values carried into the migration form, excluding retargeted relationships. */
export const migrationFormValues = (
  preview: RecordMigrationPreview | undefined,
  inlineIssues: readonly MigrationIssue[],
) =>
  (preview?.values ?? []).filter(
    (value) =>
      !inlineIssues.some(
        (issue) =>
          issue.kind === migrationIssueKinds.relationshipTargetChanged &&
          issue.attribute_code === value.attribute_code,
      ),
  );

/**
 * The relationships a migration replaces. A retargeted one left blank was not
 * answered, and sending it empty would delete its links; the server then asks
 * for a replacement or an explicit discard instead.
 */
export const answeredMigrationRelationships = <
  Relationship extends {
    attribute_code?: string | null;
    target_record_ids: readonly string[];
  },
>(
  relationships: readonly Relationship[],
  issues: readonly MigrationIssue[],
) =>
  relationships.filter(
    (relationship) =>
      relationship.target_record_ids.length > 0 ||
      !issues.some(
        (issue) =>
          issue.kind === migrationIssueKinds.relationshipTargetChanged &&
          issue.attribute_code === relationship.attribute_code,
      ),
  );

/**
 * Human-readable value of an attribute before migration; related records are
 * named by `recordLabel`.
 */
export const describeMigrationValues = (
  values: readonly MigrationValue[],
  attributeCode: string,
  fileCountLabel: (count: number) => string,
  recordLabel: (recordId: string) => string = (recordId) => recordId,
) =>
  values
    .filter((value) => value.attribute_code === attributeCode)
    .map((value) =>
      value.kind === attributeValueKinds.scalar
        ? valueForField(value.value)
        : value.kind === attributeValueKinds.relationship
          ? recordLabel(value.target_record_id)
          : fileCountLabel(value.files.length),
    )
    .join(', ');

/** The issue in the user's language; a kind the UI does not know keeps the API's message. */
export const migrationIssueMessage = (issue: MigrationIssue, t: TFunction) =>
  t(`records.migrationIssues.${issue.kind}`, { defaultValue: issue.message });

/**
 * Lays out a migration form with the fields to review first and every other
 * target attribute in a collapsed section after them.
 */
export const migrationLayout = (
  attributes: readonly Attribute[],
  reviewCodes: ReadonlySet<string>,
  labels: { review: string; others: (count: number) => string },
): ViewDefinition => {
  const field = (attribute: Attribute) => ({
    type: 'field' as const,
    field: attribute.code,
  });
  const review = attributes.filter((attribute) =>
    reviewCodes.has(attribute.code),
  );
  const others = attributes.filter(
    (attribute) => !reviewCodes.has(attribute.code),
  );
  if (review.length === 0)
    return { type: 'stack', children: others.map(field) };
  return {
    type: 'stack',
    children: [
      { type: 'heading', text: labels.review },
      ...review.map(field),
      ...(others.length > 0
        ? [
            {
              type: 'accordion' as const,
              sections: [
                {
                  label: labels.others(others.length),
                  children: others.map(field),
                },
              ],
            },
          ]
        : []),
    ],
  };
};

export const requiredMigrationAttributes = (
  issues: readonly MigrationIssue[],
  discardAttributes: readonly string[] = [],
) =>
  issues
    .filter(
      (issue) =>
        issue.kind === migrationIssueKinds.missingRequired ||
        // Needs new links or an explicit discard, which the form can ask for
        // before the server refuses.
        (issue.kind === migrationIssueKinds.relationshipTargetChanged &&
          !discardAttributes.includes(issue.attribute_code ?? '')),
    )
    .map((issue) => issue.attribute_code)
    .filter((attributeCode): attributeCode is string => Boolean(attributeCode));

export const removedAttributeIssues = (issues: readonly MigrationIssue[]) =>
  issues.filter(
    (issue): issue is MigrationIssue & { attribute_code: string } =>
      issue.attribute_code !== null &&
      issue.kind === migrationIssueKinds.removed,
  );

/** What a migration asks the user to review on one target field. */
export type MigrationFieldReview = {
  message: string;
  /** The stored value before migration, for reference. */
  previousValue?: string;
  /** Whether the stored value is archived instead of carried over. */
  discarded?: boolean;
  /** What the field held from the stored value, restored when undone. */
  fieldValue?: string;
  /** Offered when the stored value can be archived instead of replaced. */
  onDiscardChange?: (discard: boolean) => void;
};

/** The `data-field-code` of a removal, so an error can lead to it. */
export const removalFieldCode = (attributeCode: string) =>
  `removed:${attributeCode}`;
