import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { Alert, Box, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/queryKeys';
import { defaultContextCode } from '../contexts/constants';
import { migrateRecord, previewRecordMigration } from './api';
import { RecordForm } from './components/RecordForm';
import { RecordPage } from './components/RecordPage';
import { MigrationIssueList } from './components/MigrationIssueList';
import { valuesForForm } from './recordForm';
import {
  describeMigrationValues,
  migrationFormValues,
  partitionMigrationIssues,
  requiredMigrationAttributes,
} from './migrationReview';
import { recordQueryKeys } from './queryKeys';
import { invalidateRecord } from './invalidateRecord';

export const MigrateRecordPage = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/records/$recordId/migrate' });
  const [discardAttributes, setDiscardAttributes] = useState<string[]>([]);
  const preview = useQuery({
    queryKey: recordQueryKeys.migrationPreview(recordId),
    queryFn: () => previewRecordMigration(recordId),
  });
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  const { inlineIssues, standaloneIssues } = partitionMigrationIssues(
    preview.data,
  );
  const migrationValues = migrationFormValues(preview.data, inlineIssues);
  const migrationReviewMessages = Object.fromEntries(
    inlineIssues.map((issue) => {
      const currentValue = describeMigrationValues(
        preview.data?.values ?? [],
        issue.attribute_code!,
        (count) => t('records.fileCount', { count }),
      );
      return [
        issue.attribute_code!,
        currentValue
          ? `${issue.message} ${t('records.currentValue', { value: currentValue })}`
          : issue.message,
      ];
    }),
  );
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
  return (
    <RecordPage
      blueprint={preview.data?.target.blueprint}
      title={t('records.upgradeRecord')}
    >
      <Box sx={{ mt: 1 }}>
        <Link params={{ recordId }} to="/records/$recordId">
          {t('records.backToRecord')}
        </Link>
      </Box>
      {preview.isPending && (
        <Typography sx={{ mt: 4 }}>
          {t('records.preparingMigration')}
        </Typography>
      )}
      {(preview.error || migrate.error) && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {(preview.error ?? migrate.error)?.message}
        </Alert>
      )}
      {preview.data && (
        <>
          <Typography sx={{ mt: 4 }}>
            {t('records.upgradeFrom', {
              source: preview.data.source_version,
              target: preview.data.target.blueprint.version,
            })}
          </Typography>
          <MigrationIssueList
            disabled={migrate.isPending}
            discardAttributes={discardAttributes}
            issues={standaloneIssues}
            onDiscardChange={setDiscarded}
          />
          <RecordForm
            blueprint={preview.data.target}
            expectedUpdatedAt={preview.data.source_updated_at}
            contextId={defaultContextId}
            defaultContextId={defaultContextId}
            error={migrate.error}
            existingValues={migrationValues}
            highlightedAttributes={Object.keys(migrationReviewMessages)}
            migrationReviewMessages={migrationReviewMessages}
            requiredAttributes={requiredMigrationAttributes(
              preview.data.issues,
            )}
            initialValues={valuesForForm(
              preview.data.target.attributes,
              migrationValues,
              defaultContextId,
            )}
            isLoadingBlueprint={migrate.isPending}
            onSubmit={({ values, relationships, expected_updated_at }) =>
              migrate.mutate({
                expected_updated_at,
                values,
                relationships,
                discardAttributes: [...discardAttributes],
              })
            }
            showAllAttributes
            submitLabel={t('records.upgradeRecord')}
          />
        </>
      )}
    </RecordPage>
  );
};
