import { Link, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';

/**
 * Whether the record's inline changes are saving, waiting, or all saved; a
 * waiting count leads to the first unsaved field.
 */
export const InlineSaveStatus = ({
  onReveal,
  pendingCodes,
  savedAny,
  saving,
}: {
  onReveal: (code: string) => void;
  pendingCodes: readonly string[];
  savedAny: boolean;
  saving: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <Typography
      aria-live="polite"
      color="text.secondary"
      sx={{ display: 'block', minHeight: '1.5em', mb: 1 }}
      variant="caption"
    >
      {saving ? (
        t('records.savingChanges')
      ) : pendingCodes.length > 0 ? (
        <Link
          color="inherit"
          component="button"
          onClick={() => onReveal(pendingCodes[0])}
          type="button"
          variant="caption"
        >
          {t('records.unsavedChangeCount', { count: pendingCodes.length })}
        </Link>
      ) : savedAny ? (
        t('records.allChangesSaved')
      ) : null}
    </Typography>
  );
};
