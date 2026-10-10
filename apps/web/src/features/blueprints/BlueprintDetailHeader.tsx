import { Link } from '@tanstack/react-router';
import { Box, Button, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { CircleArrowUpIcon, PencilIcon, SendIcon } from 'lucide-react';
import { PageHeader } from '../../components/PageHeader';
import { blueprintStatusChipColor, blueprintStatuses } from './constants';
import { Timestamp } from '../../time/Timestamp';
import type { Blueprint } from './schemas';
import { BlueprintIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';

export const BlueprintDetailHeader = ({
  blueprint,
  canStartSafeMigration,
  migrationActive,
  migrationStatusKnown,
  onMigrate,
  onPublish,
  onPublishRecords,
}: {
  blueprint: Blueprint;
  canStartSafeMigration: boolean;
  migrationActive: boolean;
  migrationStatusKnown: boolean;
  onMigrate: () => void;
  onPublish: () => void;
  onPublishRecords: () => void;
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
              <Button startIcon={<PencilIcon />} variant="outlined">
                {t('blueprints.editBlueprint')}
              </Button>
            </Link>
            {blueprint.status === blueprintStatuses.draft && (
              <Button
                color="primary"
                onClick={onPublish}
                startIcon={<SendIcon />}
                variant="contained"
              >
                {t('blueprints.publish')}
              </Button>
            )}
            {blueprint.status === blueprintStatuses.published && (
              <Button
                onClick={onPublishRecords}
                startIcon={<SendIcon />}
                variant="outlined"
              >
                {t('blueprints.publishRecords')}
              </Button>
            )}
            {canStartSafeMigration && (
              <Button
                disabled={!migrationStatusKnown || migrationActive}
                onClick={onMigrate}
                startIcon={<CircleArrowUpIcon />}
                variant="contained"
              >
                {migrationActive
                  ? t('blueprints.migrationInProgress')
                  : t('blueprints.migrateCompatibleRecords')}
              </Button>
            )}
          </Stack>
        }
        eyebrow={t('blueprints.blueprint')}
        icon={BlueprintIcon}
        title={lexiconText(blueprint.name)}
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
        <Timestamp
          fallback={t('blueprints.notPublished')}
          value={blueprint.updated_at}
        />
      </Typography>
    </>
  );
};
