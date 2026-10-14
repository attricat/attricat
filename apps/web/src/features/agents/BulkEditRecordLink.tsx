import { Box } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

const shortRecordIdLength = 8;

/** Links a record of a bulk edit by its label, or by a short ID. */
export const BulkEditRecordLink = ({
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
          aria-label={t('agents.bulkEdit.recordWithoutLabel', {
            id: recordId,
          })}
          component="span"
          sx={{ fontFamily: 'monospace' }}
        >
          {recordId.slice(0, shortRecordIdLength)}
        </Box>
      )}
    </Link>
  );
};
