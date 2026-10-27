import { Link } from '@tanstack/react-router';
import { Box, Button, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { CircleArrowUpIcon, PencilIcon, SendIcon } from 'lucide-react';
import { PageHeader } from '../../components/PageHeader';
import { revisionStatuses } from '../../api/revisionStatus';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { Timestamp } from '../../time/Timestamp';
import type { Blueprint } from './schemas';
import { BlueprintIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';
import { useCanOn } from '../auth/useCan';
import { capabilities, accessTargetKinds } from '../auth/capabilities';
import { monoFontFamily } from '../../app/theme';

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
  const canEdit = useCanOn(
    capabilities.blueprintsWrite,
    accessTargetKinds.blueprint,
    blueprint.id,
  );
  const canPublish = useCanOn(
    capabilities.blueprintsPublish,
    accessTargetKinds.blueprint,
    blueprint.id,
  );
  const canPublishRecords = useCanOn(
    capabilities.recordsPublish,
    accessTargetKinds.blueprint,
    blueprint.id,
  );
  return (
    <>
      <PageHeader
        actions={
          <Stack direction="row" spacing={1}>
            {canEdit && (
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
            )}
            {canPublish && blueprint.status === revisionStatuses.draft && (
              <Button
                color="primary"
                onClick={onPublish}
                startIcon={<SendIcon />}
                variant="contained"
              >
                {t('blueprints.publish')}
              </Button>
            )}
            {canPublishRecords &&
              blueprint.status === revisionStatuses.published && (
                <Button
                  onClick={onPublishRecords}
                  startIcon={<SendIcon />}
                  variant="outlined"
                >
                  {t('blueprints.publishRecords')}
                </Button>
              )}
            {canPublish && canStartSafeMigration && (
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
        <RevisionStatusChip status={blueprint.status} />
        <Chip
          label={t('blueprints.latestVersion', { version: blueprint.version })}
        />
      </Stack>
      <Typography color="text.secondary" sx={{ mt: 1.5 }}>
        {t('blueprints.id')}:{' '}
        <Box component="span" sx={{ fontFamily: monoFontFamily }}>
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
