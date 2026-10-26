import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Box, Skeleton, Stack, Typography } from '@mui/material';
import { ArrowLeftIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { defaultContextCode } from '../contexts/constants';
import { migrateRecord, previewRecordMigration } from './api';
import { RecordForm } from './components/RecordForm';
import { RecordPage } from './components/RecordPage';
import { MigrationIssueList } from './components/MigrationIssueList';
import { attributeLabel } from './recordDisplay';
import { valuesForForm } from './recordForm';
import {
  describeMigrationValues,
  migrationFormValues,
  migrationIssueMessage,
  migrationLayout,
  partitionMigrationIssues,
  removalFieldCode,
  removedAttributeIssues,
  requiredMigrationAttributes,
  type MigrationFieldReview,
} from './migrationReview';
import { recordQueryKeys } from './queryKeys';
import { invalidateRecord } from './invalidateRecord';
import { useRecordLabels } from './useRecordLabels';
import { attributeValueKinds } from './valueTypes';
import { blueprintByCodeOptions } from './queryOptions';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { contextListOptions } from '../contexts/queryOptions';

export const MigrateRecordPage = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/records/$recordId/migrate' });
  const [discardAttributes, setDiscardAttributes] = useState<string[]>([]);
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
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  const { inlineIssues, standaloneIssues } = partitionMigrationIssues(
    preview.data,
  );
  const migrationValues = migrationFormValues(preview.data, inlineIssues);
  const previousValue = (attributeCode: string) =>
    describeMigrationValues(
      values,
      attributeCode,
      (count) => t('records.fileCount', { count }),
      (id) => recordLabels.get(id) ?? id,
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
    const stored = previousValue(code) || undefined;
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
  const pendingConfirmations = removals
    .filter((issue) => !discardAttributes.includes(issue.attribute_code))
    .map((issue) => ({
      id: removalFieldCode(issue.attribute_code),
      label: sourceAttributeLabel(issue.attribute_code),
      message: t('records.confirmDiscard'),
    }));
  const setDiscarded = (attributeCode: string, discard: boolean) =>
    setDiscardAttributes((attributes) =>
      discard
        ? [...attributes, attributeCode]
        : attributes.filter((attribute) => attribute !== attributeCode),
    );
  const migrate = useMutation({
    mutationFn: ({
      expected_updated_at,
      values,
      relationships,
      discardAttributes,
    }: {
      expected_updated_at?: string;
      values: Parameters<typeof migrateRecord>[1]['values'];
      relationships: Parameters<typeof migrateRecord>[1]['relationships'];
      discardAttributes: string[];
    }) => {
      if (!preview.data) throw new Error(t('records.loadMigrationFirst'));
      return migrateRecord(recordId, {
        expected_updated_at,
        migration_id: preview.data.migration_id,
        expected_target_version: preview.data.target.blueprint.version,
        values,
        relationships,
        discard_attributes: discardAttributes,
      });
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
      {preview.isPending && (
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
      <ApiErrorAlert error={preview.error} sx={{ mt: 4 }} />
      {preview.data && (
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
          requiredAttributes={requiredMigrationAttributes(preview.data.issues)}
          initialValues={initialFields}
          isSubmitting={migrate.isPending}
          onCancel={() =>
            void navigate({
              to: '/records/$recordId',
              params: { recordId },
            })
          }
          onSubmit={({ values, relationships, expected_updated_at }) =>
            migrate.mutate({
              expected_updated_at,
              values,
              relationships,
              discardAttributes: [...discardAttributes],
            })
          }
          submitLabel={t('records.upgradeRecord')}
        >
          <Box>
            <Typography sx={{ fontWeight: 600 }}>
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
        </RecordForm>
      )}
    </RecordPage>
  );
};
