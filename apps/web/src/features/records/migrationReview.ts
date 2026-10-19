import type { TFunction } from 'i18next';
import type { Attribute, RecordMigrationPreview, ViewDefinition } from './api';
import { valueForField } from './attributeValues';
import { migrationIssueKinds } from './constants';
import { attributeValueKinds } from './valueTypes';

export type MigrationIssue = RecordMigrationPreview['issues'][number];
type MigrationValue = RecordMigrationPreview['values'][number];

/** Splits migration issues into those shown on target fields and the rest. */
export const partitionMigrationIssues = (
  preview: RecordMigrationPreview | undefined,
) => {
  const targetAttributeCodes = new Set(
    preview?.target.attributes.map((attribute) => attribute.code) ?? [],
  );
  const issues = preview?.issues ?? [];
  const inlineIssues = issues.filter(
    (issue) =>
      issue.attribute_code !== null &&
      targetAttributeCodes.has(issue.attribute_code),
  );
  return {
    inlineIssues,
    standaloneIssues: issues.filter((issue) => !inlineIssues.includes(issue)),
  };
};

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
) =>
  issues
    .filter((issue) => issue.kind === migrationIssueKinds.missingRequired)
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
