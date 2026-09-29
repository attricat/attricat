import {
  Alert,
  Box,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { listRuleRuns } from './api';

export const RunsSection = ({
  error,
  runs,
}: {
  error: boolean;
  runs: Awaited<ReturnType<typeof listRuleRuns>> | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('rules.errors.runs')}
        </Alert>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ p: 2 }}>
          <Typography variant="h6">{t('rules.runHistory')}</Typography>
        </Box>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.source')}</TableCell>
              <TableCell>{t('rules.columns.status')}</TableCell>
              <TableCell>{t('rules.columns.evaluated')}</TableCell>
              <TableCell>{t('rules.columns.findings')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(runs ?? []).map((item) => (
              <TableRow key={item.id}>
                <TableCell>
                  {item.dry_run
                    ? t('rules.dryRunSuffix', { source: item.source })
                    : item.source}
                </TableCell>
                <TableCell>
                  {t(`rules.runStatuses.${item.status}`, {
                    defaultValue: item.status,
                  })}
                </TableCell>
                <TableCell>{item.candidates_evaluated}</TableCell>
                <TableCell>{item.findings_created}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
    </>
  );
};
