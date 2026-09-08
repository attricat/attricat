import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Checkbox,
  FormControlLabel,
  FormGroup,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { migrateEntity, previewEntityMigration } from './api';
import { valueForField } from './attribute-values';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

export const MigrateEntityPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/entities/$entityId/migrate' });
  const [discardAttributes, setDiscardAttributes] = useState<string[]>([]);
  const preview = useQuery({
    queryKey: entityQueryKeys.migrationPreview(entityId),
    queryFn: () => previewEntityMigration(entityId),
  });
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: listContexts,
  });
  const defaultContextId =
    contexts.data?.find((context) => context.code === 'default')?.id ?? null;
  const targetAttributeCodes = new Set(
    preview.data?.target.attributes.map((attribute) => attribute.code) ?? [],
  );
  const inlineIssues = (preview.data?.issues ?? []).filter(
    (issue) =>
      issue.attribute_code !== null &&
      targetAttributeCodes.has(issue.attribute_code),
  );
  const standaloneIssues = (preview.data?.issues ?? []).filter(
    (issue) => !inlineIssues.includes(issue),
  );
  const migrationValues = (preview.data?.values ?? []).filter(
    (value) =>
      !inlineIssues.some(
        (issue) =>
          issue.kind === 'relationship_target_changed' &&
          issue.attribute_code === value.attribute_code,
      ),
  );
  const migrationReviewMessages = Object.fromEntries(
    inlineIssues.map((issue) => {
      const currentValue = preview
        .data!.values.filter(
          (value) => value.attribute_code === issue.attribute_code,
        )
        .map((value) =>
          value.kind === 'scalar'
            ? valueForField(value.value)
            : value.kind === 'relationship'
              ? value.target_entity_id
              : t('entities.fileCount', { count: value.files.length }),
        )
        .join(', ');
      return [
        issue.attribute_code!,
        `${issue.message}${currentValue ? ` ${t('entities.currentValue', { value: currentValue })}` : ''}`,
      ];
    }),
  );
  const migrate = useMutation({
    mutationFn: ({
      values,
      relationships,
    }: {
      values: Parameters<typeof migrateEntity>[1]['values'];
      relationships: Parameters<typeof migrateEntity>[1]['relationships'];
    }) => {
      if (!preview.data) throw new Error(t('entities.loadMigrationFirst'));
      return migrateEntity(entityId, {
        migration_id: preview.data.migration_id,
        expected_target_version: preview.data.target.blueprint.version,
        values,
        relationships,
        discard_attributes: discardAttributes,
      });
    },
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  return (
    <EntityPage title={t('entities.upgradeEntity')}>
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          {t('entities.backToEdit')}
        </Link>
      </Box>
      {preview.isPending && (
        <Typography sx={{ mt: 4 }}>
          {t('entities.preparingMigration')}
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
            {t('entities.upgradeFrom', {
              source: preview.data.source_version,
              target: preview.data.target.blueprint.version,
            })}
          </Typography>
          {standaloneIssues.map((issue) => (
            <Alert
              key={`${issue.attribute_code}:${issue.message}`}
              severity="warning"
              sx={{ mt: 2 }}
            >
              {issue.attribute_code ? `${issue.attribute_code}: ` : ''}
              {issue.message}
            </Alert>
          ))}
          <FormGroup sx={{ mt: 2 }}>
            {standaloneIssues
              .filter(
                (issue): issue is typeof issue & { attribute_code: string } =>
                  issue.attribute_code !== null && issue.kind === 'removed',
              )
              .map((issue) => (
                <FormControlLabel
                  control={
                    <Checkbox
                      checked={discardAttributes.includes(issue.attribute_code)}
                      onChange={(event) =>
                        setDiscardAttributes((attributes) =>
                          event.target.checked
                            ? [...attributes, issue.attribute_code]
                            : attributes.filter(
                                (attribute) =>
                                  attribute !== issue.attribute_code,
                              ),
                        )
                      }
                    />
                  }
                  key={issue.attribute_code}
                  label={t('entities.confirmRemoval', {
                    attribute: issue.attribute_code,
                  })}
                />
              ))}
          </FormGroup>
          <EntityForm
            blueprint={preview.data.target}
            contextId={defaultContextId}
            defaultContextId={defaultContextId}
            error={migrate.error}
            existingValues={migrationValues}
            highlightedAttributes={Object.keys(migrationReviewMessages)}
            migrationReviewMessages={migrationReviewMessages}
            requiredAttributes={preview.data.issues
              .filter((issue) => issue.kind === 'missing_required')
              .map((issue) => issue.attribute_code)
              .filter((attributeCode): attributeCode is string =>
                Boolean(attributeCode),
              )}
            initialValues={valuesForForm(
              preview.data.target.attributes,
              migrationValues,
              defaultContextId,
            )}
            isLoadingBlueprint={migrate.isPending}
            onSubmit={({ values, relationships }) =>
              migrate.mutate({ values, relationships })
            }
            showAllAttributes
            submitLabel={t('entities.upgradeEntity')}
          />
        </>
      )}
    </EntityPage>
  );
};
