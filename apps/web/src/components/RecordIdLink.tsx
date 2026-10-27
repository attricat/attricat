import { Box } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../app/theme';
import { shortId } from './shortId';

/** Links a record by its display label, or by a short ID when it has none. */
export const RecordIdLink = ({
  recordId,
  label,
}: {
  recordId: string;
  label: string | null | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <Link params={{ recordId }} title={recordId} to="/records/$recordId">
      {label || (
        <Box
          aria-label={t('common.recordWithoutLabel', { id: recordId })}
          component="span"
          sx={{ fontFamily: monoFontFamily }}
        >
          {shortId(recordId)}
        </Box>
      )}
    </Link>
  );
};
