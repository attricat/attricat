import {
  Box,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import { CircleXIcon, RotateCwIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { EmptyState } from '../../components/EmptyState';
import { NextPageButton } from '../../components/LoadMoreButton';
import { StatusChip, type StatusChipStyle } from '../../components/StatusChip';
import { ExtensionFailureIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { HandlerFailuresTable } from '../extension-exports/HandlerFailuresTable';
import { formatPushError } from '../extension-exports/pushErrors';
import { deliveryFailuresOptions, runFailuresOptions } from './queryOptions';
import {
  runFailureStates,
  type RunFailure,
  type RunFailureState,
} from './schemas';

const stateChips = {
  [runFailureStates.failed]: { color: 'error', icon: CircleXIcon },
  [runFailureStates.retrying]: { color: 'warning', icon: RotateCwIcon },
} as const satisfies Record<RunFailureState, StatusChipStyle>;

const RunFailuresTable = ({ failures }: { failures: RunFailure[] }) => {
  const { t } = useTranslation();
  return (
    <Stack spacing={1}>
      <Typography component="h3" variant="subtitle1">
        {t('extensionFailures.runs')}
      </Typography>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('extensionFailures.operation')}</TableCell>
            <TableCell>{t('extensionFailures.state')}</TableCell>
            <TableCell>{t('extensionFailures.error')}</TableCell>
            <TableCell>{t('extensionFailures.attempts')}</TableCell>
            <TableCell>{t('extensionFailures.updated')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {failures.map((failure) => (
            <TableRow key={failure.id}>
              <TableCell>
                {failure.operation_id}
                {failure.scheduled && (
                  <Typography color="text.secondary" variant="body2">
                    {t('extensionFailures.scheduled')}
                  </Typography>
                )}
              </TableCell>
              <TableCell>
                <StatusChip
                  label={t(`extensionFailures.states.${failure.state}`)}
                  style={stateChips[failure.state]}
                />
              </TableCell>
              <TableCell sx={{ overflowWrap: 'anywhere' }}>
                {formatPushError({
                  code: failure.error_code,
                  message: failure.error_message,
                })}
              </TableCell>
              <TableCell>{failure.attempts}</TableCell>
              <TableCell>
                <Timestamp value={failure.updated_at} />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Stack>
  );
};

/**
 * Background work of the extension that failed: event deliveries its
 * handlers rejected or gave up on, and operation runs, scheduled ones
 * included, that failed or wait to retry. Each shows the extension's own
 * error code and message.
 */
export const ExtensionFailuresSection = ({
  extensionId,
}: {
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const runs = useInfiniteQuery(runFailuresOptions(extensionId));
  const deliveries = useInfiniteQuery(deliveryFailuresOptions(extensionId));
  const runFailures = runs.data?.pages.flatMap((page) => page.items) ?? [];
  const handlerFailures =
    deliveries.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" sx={{ mb: 1 }} variant="h6">
        {t('extensionFailures.title')}
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }} variant="body2">
        {t('extensionFailures.description')}
      </Typography>
      <ApiErrorAlert error={runs.error ?? deliveries.error} />
      {runs.data &&
        deliveries.data &&
        !handlerFailures.length &&
        !runFailures.length && (
          <EmptyState
            description={t('extensionFailures.emptyDescription')}
            icon={ExtensionFailureIcon}
            title={t('extensionFailures.empty')}
          />
        )}
      <Stack spacing={3}>
        {runFailures.length > 0 && (
          <Box>
            <RunFailuresTable failures={runFailures} />
            <NextPageButton query={runs} sx={{ mt: 2 }} />
          </Box>
        )}
        {handlerFailures.length > 0 && (
          <Box>
            <HandlerFailuresTable failures={handlerFailures} />
            <NextPageButton query={deliveries} sx={{ mt: 2 }} />
          </Box>
        )}
      </Stack>
    </Paper>
  );
};
