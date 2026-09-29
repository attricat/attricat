import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { listRules } from './api';

export const RulesSection = ({
  canManage,
  error,
  onRun,
  running,
  rules,
}: {
  canManage: boolean;
  error: boolean;
  onRun: (id: string, dryRun: boolean) => void;
  running: boolean;
  rules: Awaited<ReturnType<typeof listRules>> | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('rules.errors.definitions')}
        </Alert>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ overflowX: 'auto' }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('rules.columns.rule')}</TableCell>
                <TableCell>{t('rules.columns.revision')}</TableCell>
                <TableCell>{t('rules.columns.lifecycle')}</TableCell>
                <TableCell>{t('rules.columns.actions')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(rules ?? []).map((rule) => (
                <TableRow key={`${rule.id}-${rule.version}`}>
                  <TableCell>
                    <Stack spacing={0.25}>
                      <Typography>{rule.name}</Typography>
                      <Typography color="text.secondary" variant="caption">
                        {rule.code}
                      </Typography>
                    </Stack>
                  </TableCell>
                  <TableCell>v{rule.version}</TableCell>
                  <TableCell>
                    <Chip
                      color={rule.enabled_version ? 'success' : 'default'}
                      label={
                        rule.enabled_version
                          ? t('rules.enabled')
                          : t('rules.disabled')
                      }
                      size="small"
                    />
                  </TableCell>
                  <TableCell>
                    {canManage && (
                      <>
                        <Button
                          disabled={running}
                          size="small"
                          onClick={() => onRun(rule.id, true)}
                        >
                          {t('rules.dryRun')}
                        </Button>
                        <Button
                          disabled={running}
                          size="small"
                          onClick={() => onRun(rule.id, false)}
                        >
                          {t('rules.runNow')}
                        </Button>
                      </>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Box>
      </Paper>
    </>
  );
};
