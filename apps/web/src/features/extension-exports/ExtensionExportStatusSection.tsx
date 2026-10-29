import { extensionConnectionsOptions } from './queryOptions';
import {
  Box,
  Button,
  CircularProgress,
  FormControl,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Typography,
} from '@mui/material';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { RotateCcwIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { NextPageButton } from '../../components/LoadMoreButton';
import { ExportIcon } from '../../components/systemIcons';
import { useCan } from '../auth/useCan';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { extensionExportStatus } from './api';
import {
  exportStatuses,
  initialExportStatusFilter,
  retryableExportStatuses,
  type ExportStatus,
  exportFilterMinWidth,
} from './constants';
import { extensionExportQueryKeys } from './queryKeys';
import type { ManifestExports } from './schemas';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { allConnections, countsByConnection } from './exportCounts';
import { ExportCountsTable } from './ExportCountsTable';
import { ExportMappingsTable } from './ExportMappingsTable';
import { HandlerFailuresTable } from './HandlerFailuresTable';
import {
  supportedOutletContextVersion,
  extensionOutlets,
} from '../extensions/constants';
import { useRetryExports } from './useRetryExports';
import { RetryResultAlerts } from './RetryResultAlerts';

/**
 * An export extension's destinations: per-connection status counts, the
 * records in one status with retry, failed handler deliveries and the
 * destination's mapping editor.
 */
export const ExtensionExportStatusSection = ({
  canManage,
  extensionId,
  manifest,
}: {
  canManage: boolean;
  extensionId: string;
  manifest: ManifestExports;
}) => {
  const { t } = useTranslation();
  // The status lists records across the workspace, so it needs
  // workspace-wide read access.
  const canReadRecords = useCan(capabilities.recordsRead);
  const exports = canReadRecords ? (manifest.server?.exports ?? []) : [];
  // Until an operator picks one, show the first connection with exports.
  const [chosenConnection, setConnection] = useState<string | null>();
  const [status, setStatus] = useState<ExportStatus>(initialExportStatusFilter);
  const [selected, setSelected] = useState<string[]>([]);
  const overview = useQuery({
    queryKey: extensionExportQueryKeys.extension(extensionId, {
      connection: null,
      status: null,
    }),
    queryFn: () =>
      // Only the counts are read, so one mapping row is enough.
      extensionExportStatus(extensionId, {
        connection: null,
        status: null,
        limit: 1,
      }),
    enabled: exports.length > 0,
  });
  // A connection without exports yet can be picked too.
  const connections = useQuery({
    ...extensionConnectionsOptions(extensionId),
    enabled: exports.length > 0,
  });
  const connection =
    chosenConnection !== undefined
      ? chosenConnection
      : (overview.data?.counts.find((count) => count.connection)?.connection ??
        null);
  const filter = { connection, status };
  const page = useInfiniteQuery({
    queryKey: extensionExportQueryKeys.extensionPages(extensionId, filter),
    queryFn: ({ pageParam }) =>
      extensionExportStatus(extensionId, { ...filter, after: pageParam }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next,
    enabled: exports.length > 0 && overview.isSuccess,
  });
  const retry = useRetryExports(() => setSelected([]));
  const retryRecords = (recordIds?: string[]) =>
    retry.mutate({
      extensionId,
      selection: recordIds ? { connection, recordIds } : { connection, status },
    });
  if (!exports.length) return null;
  const rows = countsByConnection(overview.data?.counts ?? []);
  const mappings = page.data?.pages.flatMap((item) => item.mappings) ?? [];
  const handlerFailures = page.data?.pages[0]?.handler_failures ?? [];
  const retryable = retryableExportStatuses.includes(status);
  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensionExports.sectionTitle')}</Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }} variant="body2">
        {t('extensionExports.sectionDescription', {
          destinations: exports.map((item) => item.title).join(', '),
        })}
      </Typography>
      <ApiErrorAlert error={overview.error} />
      <ApiErrorAlert error={connections.error} />
      <ApiErrorAlert error={page.error} />
      <RetryResultAlerts retry={retry} />
      {overview.isPending && (
        <CircularProgress aria-label={t('extensionExports.loading')} />
      )}
      {overview.isSuccess &&
        (rows.length === 0 ? (
          <EmptyState
            description={t('extensionExports.nothingExportedDescription')}
            icon={ExportIcon}
            title={t('extensionExports.nothingExported')}
          />
        ) : (
          <ExportCountsTable rows={rows} />
        ))}
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ mt: 3, mb: 1 }}
      >
        <FormControl sx={{ minWidth: exportFilterMinWidth }}>
          <InputLabel id={`${extensionId}-export-connection`} shrink>
            {t('extensionExports.connection')}
          </InputLabel>
          <Select
            displayEmpty
            label={t('extensionExports.connection')}
            labelId={`${extensionId}-export-connection`}
            value={connection ?? allConnections}
            onChange={(event) => {
              setSelected([]);
              setConnection(event.target.value || null);
            }}
          >
            <MenuItem value={allConnections}>
              {t('extensionExports.defaultConnection')}
            </MenuItem>
            {[
              ...new Set([
                ...rows.flatMap((row) =>
                  row.connection ? [row.connection] : [],
                ),
                ...(connections.data ?? []).map((item) => item.connection),
              ]),
            ].map((name) => (
              <MenuItem key={name} value={name}>
                {name}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <FormControl sx={{ minWidth: exportFilterMinWidth }}>
          <InputLabel id={`${extensionId}-export-status`}>
            {t('extensionExports.statusColumn')}
          </InputLabel>
          <Select
            label={t('extensionExports.statusColumn')}
            labelId={`${extensionId}-export-status`}
            value={status}
            onChange={(event) => {
              setSelected([]);
              setStatus(event.target.value as ExportStatus);
            }}
          >
            {exportStatuses.map((item) => (
              <MenuItem key={item} value={item}>
                {t(`extensionExports.status.${item}`)}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        {canManage && retryable && (
          <>
            <Button
              disabled={!selected.length || retry.isPending}
              startIcon={<RotateCcwIcon />}
              onClick={() => retryRecords(selected)}
            >
              {t('extensionExports.retrySelected', { count: selected.length })}
            </Button>
            <Button
              disabled={!mappings.length || retry.isPending}
              startIcon={<RotateCcwIcon />}
              onClick={() => retryRecords()}
            >
              {t('extensionExports.retryAll', {
                status: t(`extensionExports.status.${status}`),
              })}
            </Button>
          </>
        )}
      </Stack>
      {page.isPending && overview.isSuccess && (
        <CircularProgress aria-label={t('extensionExports.loading')} />
      )}
      {page.isSuccess &&
        (mappings.length === 0 ? (
          <Typography color="text.secondary" variant="body2">
            {t('extensionExports.noRecordsInStatus')}
          </Typography>
        ) : (
          <ExportMappingsTable
            mappings={mappings}
            onSelectedChange={setSelected}
            selected={canManage && retryable ? selected : undefined}
          />
        ))}
      <NextPageButton query={page} sx={{ mt: 2 }} />
      {handlerFailures.length > 0 && (
        <HandlerFailuresTable failures={handlerFailures} />
      )}
      {exports.map((destination) => (
        <Box key={destination.id} sx={{ mt: 3 }}>
          <ExtensionOutlet
            context={{
              context_version: supportedOutletContextVersion,
              export_id: destination.id,
            }}
            extensionId={extensionId}
            outlet={extensionOutlets.exportMapping}
          />
        </Box>
      ))}
    </Paper>
  );
};
