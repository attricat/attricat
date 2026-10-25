import { Alert, Box, Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../api';
import { conflictResolutions, type ConflictResolution } from '../constants';
import { ConflictingChange } from './ConflictingChange';

/**
 * Another editor saved while this one's changes were pending: lists each
 * field's two values and lets the user keep theirs or take the latest.
 */
export const SaveConflictAlert = ({
  attributes,
  latest,
  onResolve,
  pending,
}: {
  /** The fields with a pending change. */
  attributes: readonly Attribute[];
  latest: Readonly<Record<string, string>>;
  onResolve: (resolution: ConflictResolution) => void;
  pending: Readonly<Record<string, string>>;
}) => {
  const { t } = useTranslation();
  return (
    <Alert
      action={
        <Box sx={{ display: 'flex', gap: 1 }}>
          <Button
            color="inherit"
            onClick={() => onResolve(conflictResolutions.keepMine)}
            size="small"
          >
            {t('records.keepMyChanges')}
          </Button>
          <Button
            color="inherit"
            onClick={() => onResolve(conflictResolutions.useTheirs)}
            size="small"
          >
            {t('records.useLatestValues')}
          </Button>
        </Box>
      }
      severity="warning"
      sx={{ mb: 2 }}
    >
      {t('records.changedByAnotherEditor')}
      <Box component="ul" sx={{ mb: 0, mt: 1, pl: 2 }}>
        {attributes.map((attribute) => (
          <li key={attribute.code}>
            <ConflictingChange
              attribute={attribute}
              mine={pending[attribute.code] ?? ''}
              theirs={latest[attribute.code] ?? ''}
            />
          </li>
        ))}
      </Box>
    </Alert>
  );
};
