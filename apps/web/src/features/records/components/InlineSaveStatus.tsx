import { Link, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useRevealField } from '../../views/fieldReveal';

/**
 * Whether the record's inline changes are saving, waiting, or all saved; a
 * waiting count leads to the first unsaved field where the page reveals
 * fields.
 */
export const InlineSaveStatus = ({
  pendingCodes,
  savedAny,
  saving,
}: {
  pendingCodes: readonly string[];
  savedAny: boolean;
  saving: boolean;
}) => {
  const { t } = useTranslation();
  const revealField = useRevealField();
  const unsaved = t('records.unsavedChangeCount', {
    count: pendingCodes.length,
  });
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
        revealField ? (
          <Link
            color="inherit"
            component="button"
            onClick={() => revealField(pendingCodes[0])}
            type="button"
            variant="caption"
          >
            {unsaved}
          </Link>
        ) : (
          unsaved
        )
      ) : savedAny ? (
        t('records.allChangesSaved')
      ) : null}
    </Typography>
  );
};
