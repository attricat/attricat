import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
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
    <Dialog onClose={() => !isPending && onClose()} open={open}>
      <DialogTitle>{t('blueprints.migrateCompatibleEntities')}</DialogTitle>
      <DialogContent>
        <DialogContentText>
          {t('blueprints.migrateCompatibleEntitiesDescription', {
            source: sourceVersion,
            target: targetVersion,
          })}
        </DialogContentText>
        {impact && (
          <DialogContentText sx={{ mt: 2 }}>
            {t('blueprints.migrationImpact', {
              entities: impact.eligible_entities,
              values: impact.removed_values,
              affectedEntities: impact.entities_with_removed_values,
              attributes: impact.removed_attribute_codes.join(', '),
            })}
          </DialogContentText>
        )}
        {impact?.requires_removal_disposition && (
          <Alert severity="warning" sx={{ mt: 2 }}>
            {t('blueprints.archiveRemovedValues')}
          </Alert>
        )}
        {error && <ApiErrorAlert error={error} sx={{ mt: 2 }} />}
      </DialogContent>
      <DialogActions>
        <Button disabled={isPending} onClick={onClose}>
          {t('blueprints.cancel')}
        </Button>
        <Button disabled={isPending} onClick={onConfirm} variant="contained">
          {isPending
            ? t('blueprints.startingMigration')
            : t('blueprints.startMigration')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
