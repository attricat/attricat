import { CircularProgress, Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { progressSizes } from '../../components/iconSizes';
import { RouterChip } from '../../components/RouterLink';
import { blueprintDetailTabNames } from './constants';
import { isMigrationBatchActive } from './migrationBatches';
import { blueprintMigrationBatchesOptions } from './queryOptions';

/**
 * Shown while one of the blueprint's migration batches is queued or running;
 * links to the blueprint's Migrations tab, which shows the full progress.
 */
export const BlueprintMigrationBadge = ({
  blueprintId,
}: {
  blueprintId: string;
}) => {
  const { t } = useTranslation();
  const batches = useQuery(blueprintMigrationBatchesOptions(blueprintId));
  const active = batches.data?.find(isMigrationBatchActive);
  if (!active) return null;
  return (
    <Tooltip
      title={t('blueprints.migrationBadgeTooltip', {
        processed: active.processed_records,
        total: active.total_records,
        version: active.target_version,
      })}
    >
      <RouterChip
        clickable
        color="info"
        icon={
          <CircularProgress
            aria-hidden
            color="inherit"
            size={progressSizes.compact}
          />
        }
        label={t('blueprints.migrationBadge')}
        params={{ blueprintId }}
        search={{ tab: blueprintDetailTabNames.migrations }}
        size="small"
        to="/manage/blueprints/$blueprintId"
        variant="outlined"
      />
    </Tooltip>
  );
};
