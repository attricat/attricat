import type { RecordMigrationPreview } from './api';
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

/** Human-readable current value of an attribute before migration. */
export const describeMigrationValues = (
  values: readonly MigrationValue[],
  attributeCode: string,
  fileCountLabel: (count: number) => string,
) =>
  values
    .filter((value) => value.attribute_code === attributeCode)
    .map((value) =>
      value.kind === attributeValueKinds.scalar
        ? valueForField(value.value)
        : value.kind === attributeValueKinds.relationship
          ? value.target_record_id
          : fileCountLabel(value.files.length),
    )
    .join(', ');

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
