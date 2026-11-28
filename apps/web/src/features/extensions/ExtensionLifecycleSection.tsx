import { Box, Divider, Paper, Stack, Typography } from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import {
  createdAtCursor,
  nextKeysetCursor,
  type KeysetCursor,
} from '../../api/keysetCursor';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { NextPageButton } from '../../components/LoadMoreButton';
import { UserProfileLink } from '../../components/UserProfileLink';
import {
  lifecycleOperationLabel,
  lifecycleStateLabel,
} from './extensionPageUtils';
import { extensionLifecycle } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import type { LifecycleRecord } from './schemas';
import { Timestamp } from '../../time/Timestamp';
import { personName } from '../../components/personName';
import { EmptyState } from '../../components/EmptyState';
import { ExtensionIcon } from '../../components/systemIcons';

const hasDiagnostics = (diagnostics: unknown) =>
  typeof diagnostics === 'object' &&
  diagnostics !== null &&
  Object.keys(diagnostics).length > 0;

const ExtensionLifecycleRecord = ({ record }: { record: LifecycleRecord }) => {
  const { t } = useTranslation();
  const stateLabel = (state: string | null) =>
    state === null ? t('extensions.noState') : lifecycleStateLabel(state, t);
  return (
    <Box>
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
    </Box>
  );
};

/** The installation's lifecycle records, newest first, a page at a time. */
export const ExtensionLifecycleSection = ({
  extensionId,
}: {
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const pages = useInfiniteQuery({
    queryKey: extensionManagementQueryKeys.lifecycle(extensionId),
    queryFn: ({ pageParam }) => extensionLifecycle(extensionId, pageParam),
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, createdAtCursor),
  });
  const lifecycle = pages.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" sx={{ mb: 1 }} variant="h6">
        {t('extensions.lifecycle')}
      </Typography>
      <ApiErrorAlert error={pages.error} />
      {!pages.data ? null : lifecycle.length === 0 ? (
        <EmptyState icon={ExtensionIcon} title={t('extensions.noLifecycle')} />
      ) : (
        <Stack divider={<Divider />} spacing={1}>
          {lifecycle.map((record) => (
            <ExtensionLifecycleRecord key={record.id} record={record} />
          ))}
        </Stack>
      )}
      <NextPageButton query={pages} sx={{ mt: 2 }} />
    </Paper>
  );
};
