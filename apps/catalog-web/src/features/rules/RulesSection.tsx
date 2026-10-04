import {
  Alert,
  Box,
  Button,
  Chip,
  FormControlLabel,
  Paper,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Rule } from './api';
import { RULE_STATUS_PUBLISHED } from './constants';

export const RulesSection = ({
  canManage,
  error,
  onRun,
  onToggle,
  running,
  rules,
}: {
  canManage: boolean;
  error: boolean;
  onRun: (rule: Rule, dryRun: boolean) => void;
  onToggle: (rule: Rule, enabled: boolean) => void;
  running: boolean;
  rules: readonly Rule[] | undefined;
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
              {(rules ?? []).map((rule) => {
                // Rows are revisions; only the enabled one is enabled.
                const enabled = rule.enabled_version === rule.version;
                const lifecycle = (
                  <Chip
                    color={enabled ? 'success' : 'default'}
                    label={
                      enabled
                        ? t('rules.enabled')
                        : rule.status === RULE_STATUS_PUBLISHED
                          ? t('rules.disabled')
                          : t('rules.draft')
                    }
                    size="small"
                  />
                );
                return (
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
                      {canManage && rule.status === RULE_STATUS_PUBLISHED ? (
                        <FormControlLabel
                          control={
                            <Switch
                              checked={enabled}
                              disabled={running}
                              onChange={(_, checked) => onToggle(rule, checked)}
                              slotProps={{
                                input: {
                                  'aria-label': t('rules.toggleRule', {
                                    name: rule.name,
                                    version: rule.version,
                                  }),
                                },
                              }}
                            />
                          }
                          label={lifecycle}
                        />
                      ) : (
                        lifecycle
                      )}
                    </TableCell>
                    <TableCell>
                      {canManage && (
                        <>
                          <Button
                            disabled={running}
                            size="small"
                            onClick={() => onRun(rule, true)}
                          >
                            {t('rules.dryRun')}
                          </Button>
                          <Button
                            disabled={running}
                            size="small"
                            onClick={() => onRun(rule, false)}
                          >
                            {t('rules.runNow')}
                          </Button>
                        </>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Box>
      </Paper>
    </>
  );
};
