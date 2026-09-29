import { Link } from '@tanstack/react-router';
import { Box, Button, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageHeader } from '../../components/PageHeader';
import { blueprintStatusChipColor, blueprintStatuses } from './constants';
import { formatBlueprintDateTime } from './dateTime';
import type { Blueprint } from './schemas';

export const BlueprintDetailHeader = ({
  blueprint,
  canStartSafeMigration,
  migrationActive,
  migrationStatusKnown,
  onMigrate,
  onPublish,
  onPublishEntities,
}: {
  blueprint: Blueprint;
  canStartSafeMigration: boolean;
  migrationActive: boolean;
  migrationStatusKnown: boolean;
  onMigrate: () => void;
  onPublish: () => void;
  onPublishEntities: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <>
      <PageHeader
        actions={
          <Stack direction="row" spacing={1}>
            <Link
              params={{
                blueprintId: blueprint.id,
                version: String(blueprint.version),
              }}
              to="/manage/blueprints/$blueprintId/revisions/$version/new"
            >
              <Button variant="outlined">
                {t('blueprints.editBlueprint')}
              </Button>
            </Link>
            {blueprint.status === blueprintStatuses.draft && (
              <Button color="primary" onClick={onPublish} variant="contained">
                {t('blueprints.publish')}
              </Button>
            )}
            {blueprint.status === blueprintStatuses.published && (
              <Button onClick={onPublishEntities} variant="outlined">
                {t('blueprints.publishEntities')}
              </Button>
            )}
            {canStartSafeMigration && (
              <Button
                disabled={!migrationStatusKnown || migrationActive}
                onClick={onMigrate}
                variant="contained"
              >
                {migrationActive
                  ? t('blueprints.migrationInProgress')
                  : t('blueprints.migrateCompatibleEntities')}
              </Button>
            )}
          </Stack>
        }
        eyebrow={t('blueprints.blueprint')}
        title={blueprint.name}
      />
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
        <Chip label={blueprint.code} variant="outlined" />
        <Chip label={blueprint.kind} variant="outlined" />
        <Chip
          color={blueprintStatusChipColor(blueprint.status)}
          label={t(`blueprints.revisionStatuses.${blueprint.status}`)}
        />
        <Chip
          label={t('blueprints.latestVersion', { version: blueprint.version })}
        />
      </Stack>
      <Typography color="text.secondary" sx={{ mt: 1.5 }}>
        {t('blueprints.id')}:{' '}
        <Box component="span" sx={{ fontFamily: 'monospace' }}>
          {blueprint.id}
        </Box>
        {' · '}
        {t('blueprints.updated')}:{' '}
        {formatBlueprintDateTime(
          blueprint.updated_at,
          t('blueprints.notPublished'),
        )}
      </Typography>
    </>
  );
};
