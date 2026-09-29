import { Box, Divider, Paper, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  lifecycleOperationLabel,
  lifecycleStateLabel,
} from './extensionPageUtils';
import type { ExtensionDetail } from './managementApi';

type LifecycleRecord = ExtensionDetail['lifecycle'][number];

const hasDiagnostics = (diagnostics: unknown) =>
  typeof diagnostics === 'object' &&
  diagnostics !== null &&
  Object.keys(diagnostics).length > 0;

const ExtensionLifecycleRecord = ({ record }: { record: LifecycleRecord }) => {
  const { t, i18n } = useTranslation();
  const stateLabel = (state: string | null) =>
    state === null ? t('extensions.noState') : lifecycleStateLabel(state, t);
  const details = [
    new Date(record.created_at).toLocaleString(
      i18n.resolvedLanguage ?? i18n.language,
    ),
    record.actor_user_id
      ? t('extensions.lifecycleActor', { actor: record.actor_user_id })
      : undefined,
    hasDiagnostics(record.diagnostics)
      ? t('extensions.lifecycleDiagnostics', {
          diagnostics: JSON.stringify(record.diagnostics),
        })
      : undefined,
  ].filter(Boolean);
  return (
    <Box sx={{ py: 1 }}>
      <Typography>
        {t('extensions.lifecycleTransition', {
          operation: lifecycleOperationLabel(record.operation, t),
          prior: stateLabel(record.prior_state),
          next: stateLabel(record.new_state),
        })}
      </Typography>
      <Typography color="text.secondary" variant="body2">
        {details.join(' ')}
      </Typography>
      <Divider />
    </Box>
  );
};

export const ExtensionLifecycleSection = ({
  lifecycle,
}: {
  lifecycle: LifecycleRecord[];
}) => {
  const { t } = useTranslation();
  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensions.lifecycle')}</Typography>
      {lifecycle.length === 0 ? (
        <Typography color="text.secondary">
          {t('extensions.noLifecycle')}
        </Typography>
      ) : (
        lifecycle.map((record) => (
          <ExtensionLifecycleRecord key={record.id} record={record} />
        ))
      )}
    </Paper>
  );
};
