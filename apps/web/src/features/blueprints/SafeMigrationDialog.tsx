import { Alert, Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import type { BlueprintMigrationImpact } from './schemas';

export const SafeMigrationDialog = ({
  error,
  impact,
  isPending,
  onClose,
  onConfirm,
  open,
  sourceVersion,
  targetVersion,
}: {
  error: Error | null;
  impact: BlueprintMigrationImpact | undefined;
  isPending: boolean;
  onClose: () => void;
  onConfirm: () => void;
  open: boolean;
  sourceVersion: number | undefined;
  targetVersion: number | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <ConfirmDialog
      confirmLabel={
        isPending
          ? t('blueprints.startingMigration')
          : t('blueprints.startMigration')
      }
      error={error}
      isPending={isPending}
      maxWidth="sm"
      onClose={onClose}
      onConfirm={onConfirm}
      open={open}
      title={t('blueprints.migrateCompatibleRecords')}
    >
      {t('blueprints.migrateCompatibleRecordsDescription', {
        source: sourceVersion,
        target: targetVersion,
      })}
      {impact && (
        <Box sx={{ mt: 2 }}>
          {t('blueprints.migrationImpact', {
            records: impact.eligible_records,
            values: impact.removed_values,
            affectedRecords: impact.records_with_removed_values,
            attributes: impact.removed_attribute_codes.join(', '),
          })}
        </Box>
      )}
      {impact?.requires_removal_disposition && (
        <Alert severity="warning" sx={{ mt: 2 }}>
          {t('blueprints.archiveRemovedValues')}
        </Alert>
      )}
    </ConfirmDialog>
  );
};
