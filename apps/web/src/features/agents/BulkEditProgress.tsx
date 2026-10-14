import { LinearProgress, Paper, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { BulkProgress } from './bulkEdits';

/** How far an approved bulk edit has got while it runs. */
export const BulkEditProgress = ({ progress }: { progress: BulkProgress }) => {
  const { t } = useTranslation();
  const label = t('agents.bulkEdit.progress', {
    applied: progress.applied,
    count: progress.total,
  });
  return (
    <Paper sx={{ p: 2.5 }} variant="outlined">
      <Typography id={`bulk-progress-${progress.tool_call_id}`} sx={{ mb: 1 }}>
        {label}
      </Typography>
      <LinearProgress
        aria-labelledby={`bulk-progress-${progress.tool_call_id}`}
        value={(progress.applied / progress.total) * 100}
        variant="determinate"
      />
    </Paper>
  );
};
