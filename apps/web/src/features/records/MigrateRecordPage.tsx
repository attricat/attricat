import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Box, Skeleton, Stack, Typography } from '@mui/material';
import { ArrowLeftIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { contextLabel, findDefaultContextId } from '../contexts/defaultContext';
import { migrateRecord, previewRecordMigration } from './api';
import { RecordForm } from './components/RecordForm';
import { RecordPage } from './components/RecordPage';
import { MigrationContextValues } from './components/MigrationContextValues';
import { MigrationIssueList } from './components/MigrationIssueList';
import { scalarValueForField } from './attributeValues';
import { attributeLabel } from './recordDisplay';
import { valuesForForm } from './recordForm';
import {
  describeMigrationValues,
  migrationFormValues,
  contextValueAnswered,
  migrationContextValues,
  migrationIssueMessage,
  migrationLayout,
  answeredMigrationRelationships,
  partitionMigrationIssues,
  removalFieldCode,
  removedAttributeIssues,
  requiredMigrationAttributes,
  type MigrationContextAnswer,
  type MigrationFieldReview,
} from './migrationReview';
import { recordQueryKeys } from './queryKeys';
import { invalidateRecord } from './invalidateRecord';
import { useRecordLabels } from './useRecordLabels';
import { attributeValueKinds } from './valueTypes';
import { blueprintByCodeOptions } from './queryOptions';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ApiRequestError } from '../../api/request';
import { invalidFileReferencesCode } from '../files/constants';
import {
  QueuedFileUploadsContext,
  useQueuedFileUploads,
} from '../files/queuedFileUploads';
import { contextListOptions } from '../contexts/queryOptions';

export const MigrateRecordPage = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/records/$recordId/migrate' });
  const [discardAttributes, setDiscardAttributes] = useState<string[]>([]);
  const [contextAnswers, setContextAnswers] = useState<
    Record<string, MigrationContextAnswer>
  >({});
  const preview = useQuery({
    queryKey: recordQueryKeys.migrationPreview(recordId),
    queryFn: () => previewRecordMigration(recordId),
  });
  const contexts = useQuery(contextListOptions());
  // The record's current revision names attributes the target removes.
  const sourceCode = preview.data?.target.blueprint.code;
  const sourceVersion = preview.data?.source_version;
  const source = useQuery({
    ...blueprintByCodeOptions(sourceCode ?? '', sourceVersion),
    enabled: sourceCode !== undefined && sourceVersion !== undefined,
  });
  const values = preview.data?.values ?? [];
  const recordLabels = useRecordLabels(
    values.flatMap((value) =>
      value.kind === attributeValueKinds.relationship
        ? [value.target_record_id]
        : [],
    ),
  );
  const defaultContextId = findDefaultContextId(contexts.data);
  const { inlineIssues, contextIssues, standaloneIssues } =
    partitionMigrationIssues(preview.data, defaultContextId);
  const migrationValues = migrationFormValues(preview.data, inlineIssues);
  const describeValues = (stored: typeof values, attributeCode: string) =>
    describeMigrationValues(
      stored,
      attributeCode,
      (count) => t('records.fileCount', { count }),
      (id) => recordLabels.get(id) ?? id,
    );
  const previousValue = (attributeCode: string) =>
    describeValues(values, attributeCode);
  const contextPreviousValue = (attributeCode: string, contextId: string) =>
    describeValues(
      values.filter((value) => value.context_id === contextId),
      attributeCode,
    );
  const sourceAttributeLabel = (attributeCode: string) => {
    const attribute = [
      ...(source.data?.attributes ?? []),
      ...(preview.data?.target.attributes ?? []),
    ].find((candidate) => candidate.code === attributeCode);
    return attribute ? attributeLabel(attribute) : attributeCode;
  };
  const initialFields = valuesForForm(
    preview.data?.target.attributes ?? [],
    migrationValues,
    defaultContextId,
  );
  const migrationReviews: Record<string, MigrationFieldReview> = {};
  for (const issue of inlineIssues) {
    const code = issue.attribute_code!;
    const message = migrationIssueMessage(issue, t);
    // Other contexts' values are asked about apart from this field.
    const stored =
      describeValues(
        values.filter((value) => value.context_id === defaultContextId),
        code,
      ) || undefined;
    migrationReviews[code] = {
      message: migrationReviews[code]
        ? `${migrationReviews[code].message} ${message}`
        : message,
      previousValue: stored,
      // A stored value the new revision cannot take can be archived instead
      // of replaced, which the API accepts for any attribute.
      ...(stored && {
        discarded: discardAttributes.includes(code),
        fieldValue: initialFields[code] ?? '',
        onDiscardChange: (discard: boolean) => setDiscarded(code, discard),
      }),
    };
  }
  const reviewCodes = new Set(Object.keys(migrationReviews));
  const removals = removedAttributeIssues(standaloneIssues);
  // Discarding an attribute archives its values in every context too.
  const contextValues = migrationContextValues(
    contextIssues,
    preview.data?.target.attributes ?? [],
    t,
  ).filter((value) => !discardAttributes.includes(value.attributeCode));
  const contextName = (contextId: string) => {
    const code = contexts.data?.find(({ id }) => id === contextId)?.code;
    return code ? contextLabel(t, code) : contextId;
  };
  const pendingConfirmations = [
    ...removals
      .filter((issue) => !discardAttributes.includes(issue.attribute_code))
      .map((issue) => ({
        id: removalFieldCode(issue.attribute_code),
        label: sourceAttributeLabel(issue.attribute_code),
        message: t('records.confirmDiscard'),
      })),
    ...contextValues
      .filter(
        (value) => !contextValueAnswered(value, contextAnswers[value.key]),
      )
      .map((value) => ({
        id: value.key,
        label: t('records.migrationContextValue', {
          attribute: sourceAttributeLabel(value.attributeCode),
          context: contextName(value.contextId),
        }),
        message: t('records.migrationContextValuePending'),
      })),
  ];
  const contextSubmission = () => {
    const replacements: Parameters<typeof migrateRecord>[1]['values'] = [];
    const removed: { attribute_code: string; context_id: string }[] = [];
    for (const value of contextValues) {
      const answer = contextAnswers[value.key];
      if (answer?.remove) {
        removed.push({
          attribute_code: value.attributeCode,
          context_id: value.contextId,
        });
        continue;
      }
      const attribute = preview.data?.target.attributes.find(
        (candidate) => candidate.code === value.attributeCode,
      );
      const scalar =
        attribute && scalarValueForField(attribute, answer?.text ?? '');
      if (scalar) replacements.push({ ...scalar, context_id: value.contextId });
    }
    return { replacements, removed };
  };
  const setDiscarded = (attributeCode: string, discard: boolean) =>
    setDiscardAttributes((attributes) =>
      discard
        ? [...attributes, attributeCode]
        : attributes.filter((attribute) => attribute !== attributeCode),
    );
  // A new revision's required file can only be supplied with the upgrade.
  const queuedFiles = useQueuedFileUploads(
    preview.data &&
      `${preview.data.target.blueprint.id}:${preview.data.target.blueprint.version}`,
  );
  const migrate = useMutation({
    mutationFn: async ({
      expected_updated_at,
      values,
      relationships,
      discardAttributes,
      removeValues,
    }: {
      expected_updated_at?: string;
      values: Parameters<typeof migrateRecord>[1]['values'];
      relationships: Parameters<typeof migrateRecord>[1]['relationships'];
      discardAttributes: string[];
      removeValues: { attribute_code: string; context_id: string | null }[];
    }) => {
      if (!preview.data) throw new Error(t('records.loadMigrationFirst'));
      const staged = await queuedFiles.stageQueued(
        queuedFiles.queue.pending,
        preview.data.target.blueprint.id,
        defaultContextId,
      );
      if (staged.failed.length > 0)
        throw new Error(
          t('files.uploadBeforeUpgradeFailed', {
            count: staged.failed.length,
            files: staged.failed
              .map(({ filename, message }) =>
                message ? `${filename} (${message})` : filename,
              )
              .join(', '),
          }),
        );
      try {
        return await migrateRecord(recordId, {
          expected_updated_at,
          migration_id: preview.data.migration_id,
          expected_target_version: preview.data.target.blueprint.version,
          values,
          relationships,
          discard_attributes: discardAttributes,
          remove_values: removeValues,
          files: staged.staged,
        });
      } catch (error) {
        // Files that can no longer be claimed upload again on the next attempt.
        if (
          error instanceof ApiRequestError &&
          error.code === invalidFileReferencesCode
        )
          queuedFiles.forgetStaged();
        throw error;
      }
    },
    onSuccess: async (record) => {
      // Leave first: a completed migration has no preview (409), and the
      // still-mounted preview query would otherwise be refetched and retried.
      await navigate({
        to: '/records/$recordId',
        params: { recordId: record.id },
      });
      await invalidateRecord(client, record.id);
    },
  });
  const targetAttributes = preview.data?.target.attributes ?? [];
  const otherCount = targetAttributes.length - reviewCodes.size;
  const summary = [
    reviewCodes.size > 0 &&
      t('records.migrationFieldsToReview', { count: reviewCodes.size }),
    removals.length > 0 &&
      t('records.migrationRemovals', { count: removals.length }),
    otherCount > 0 && t('records.migrationOtherFields', { count: otherCount }),
  ].filter(Boolean);
  return (
    <RecordPage
      blueprint={preview.data?.target.blueprint}
      title={t('records.upgradeRecord')}
    >
      <Box sx={{ mt: 1 }}>
        <RouterButton
          params={{ recordId }}
          startIcon={<ArrowLeftIcon />}
          to="/records/$recordId"
        >
          {t('records.backToRecord')}
        </RouterButton>
      </Box>
      {(preview.isPending || contexts.isPending) && !preview.error && (
        <Stack
          aria-label={t('records.preparingMigration')}
          spacing={2}
          sx={{ mt: 4 }}
        >
          <Skeleton height={32} variant="rounded" width="40%" />
          <Skeleton height={48} variant="rounded" />
          <Skeleton height={48} variant="rounded" />
        </Stack>
      )}
      <ApiErrorAlert error={preview.error ?? contexts.error} sx={{ mt: 4 }} />
      {/* The form keeps its first values, and those are read per context. */}
      {preview.data && contexts.data && (
        <QueuedFileUploadsContext value={queuedFiles.queue}>
          <RecordForm
            blueprint={preview.data.target}
            expectedUpdatedAt={preview.data.source_updated_at}
            contextId={defaultContextId}
            defaultContextId={defaultContextId}
            error={migrate.error}
            existingValues={migrationValues}
            guardUnsavedChanges
            layout={migrationLayout(targetAttributes, reviewCodes, {
              review: t('records.migrationNeedsReview'),
              others: (count) =>
                t('records.migrationOtherFieldsSection', { count }),
            })}
            migrationReviews={migrationReviews}
            pendingConfirmations={pendingConfirmations}
            requiredAttributes={requiredMigrationAttributes(
              preview.data.issues,
              discardAttributes,
            )}
            initialValues={initialFields}
            isSubmitting={migrate.isPending}
            onCancel={() =>
              void navigate({
                to: '/records/$recordId',
                params: { recordId },
              })
            }
            onSubmit={({
              values,
              relationships,
              clearedAttributes,
              expected_updated_at,
            }) => {
              const { replacements, removed } = contextSubmission();
              migrate.mutate({
                expected_updated_at,
                values: [...values, ...replacements],
                relationships: answeredMigrationRelationships(
                  relationships,
                  inlineIssues,
                ),
                discardAttributes: [...discardAttributes],
                removeValues: [
                  ...clearedAttributes
                    .filter((code) => !discardAttributes.includes(code))
                    .map((attribute_code) => ({
                      attribute_code,
                      context_id: defaultContextId,
                    })),
                  ...removed,
                ],
              });
            }}
            submitLabel={t('records.upgradeRecord')}
          >
            <Box>
              <Typography sx={{ fontWeight: 'fontWeightBold' }}>
                {t('records.upgradeFrom', {
                  source: preview.data.source_version,
                  target: preview.data.target.blueprint.version,
                })}
              </Typography>
              {summary.length > 0 && (
                <Typography color="text.secondary" variant="body2">
                  {summary.join(' · ')}
                </Typography>
              )}
            </Box>
            <MigrationIssueList
              attributeLabel={sourceAttributeLabel}
              disabled={migrate.isPending}
              discardAttributes={discardAttributes}
              issues={standaloneIssues}
              onDiscardChange={setDiscarded}
              previousValue={previousValue}
              targetVersion={preview.data.target.blueprint.version}
            />
            <MigrationContextValues
              answers={contextAnswers}
              attributeLabel={sourceAttributeLabel}
              contextLabel={contextName}
              disabled={migrate.isPending}
              onAnswer={(key, answer) =>
                setContextAnswers((answers) => ({ ...answers, [key]: answer }))
              }
              previousValue={contextPreviousValue}
              values={contextValues}
            />
          </RecordForm>
        </QueuedFileUploadsContext>
      )}
    </RecordPage>
  );
};
