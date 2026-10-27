import { Box, Divider, Paper, Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { UserProfileLink } from '../../components/UserProfileLink';
import {
  lifecycleOperationLabel,
  lifecycleStateLabel,
} from './extensionPageUtils';
import type { ExtensionDetail } from './managementApi';
import { Timestamp } from '../../time/Timestamp';
import { personName } from '../../components/personName';

type LifecycleRecord = ExtensionDetail['lifecycle'][number];

const hasDiagnostics = (diagnostics: unknown) =>
  typeof diagnostics === 'object' &&
  diagnostics !== null &&
  Object.keys(diagnostics).length > 0;

const ExtensionLifecycleRecord = ({ record }: { record: LifecycleRecord }) => {
  const { t } = useTranslation();
  const stateLabel = (state: string | null) =>
    state === null ? t('extensions.noState') : lifecycleStateLabel(state, t);
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
        <Timestamp style="dateTimeSeconds" value={record.created_at} />
        {record.actor_user_id && (
          <>
            {' '}
            <Trans
              components={{
                actor: <UserProfileLink userId={record.actor_user_id} />,
              }}
              i18nKey="extensions.lifecycleActor"
              t={t}
              values={{
                actor: personName(
                  record.actor_display_name,
                  record.actor_email || record.actor_user_id,
                ),
              }}
            />
          </>
        )}
        {hasDiagnostics(record.diagnostics) &&
          ` ${t('extensions.lifecycleDiagnostics', {
            diagnostics: JSON.stringify(record.diagnostics),
          })}`}
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
