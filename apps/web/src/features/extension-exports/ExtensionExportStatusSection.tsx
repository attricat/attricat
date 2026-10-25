import {
  Alert,
  Box,
  Button,
  Checkbox,
  FormControl,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { RotateCcwIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { ExportIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { useCan } from '../auth/useCan';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { extensionExportStatus, listConnections, retryExports } from './api';
import {
  exportMappingExtensionOutlet,
  exportStatuses,
  retryableExportStatuses,
  type ExportStatus,
} from './constants';
import { ExportStatusChip } from './ExportStatusChip';
import { extensionExportQueryKeys } from './queryKeys';
import type { ExportStatusCount, ManifestExports } from './schemas';
import { errorMessage } from '../../api/errors';

const allConnections = '';

/** Status counts by connection, in a stable order. */
const countsByConnection = (counts: ExportStatusCount[]) => {
  const connections = [
    ...new Set(counts.map((count) => count.connection ?? allConnections)),
  ].sort();
  return connections.map((connection) => ({
    connection: connection === allConnections ? null : connection,
    counts: Object.fromEntries(
      counts
        .filter((count) => (count.connection ?? allConnections) === connection)
        .map((count) => [count.status, count.count]),
    ) as Partial<Record<ExportStatus, number>>,
  }));
};

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
  const client = useQueryClient();
  // The status lists records across the workspace, so it needs
  // workspace-wide read access.
  const canReadRecords = useCan('records_read');
  const exports = canReadRecords ? (manifest.server?.exports ?? []) : [];
  // Until an operator picks one, show the first connection with exports.
  const [chosenConnection, setConnection] = useState<string | null>();
  const [status, setStatus] = useState<ExportStatus>('rejected');
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
    queryKey: extensionExportQueryKeys.connections(extensionId),
    queryFn: () => listConnections(extensionId),
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
  const retry = useMutation({
    mutationFn: ({
      recordIds,
      key,
    }: {
      recordIds: string[] | undefined;
      key: string;
    }) =>
      retryExports(
        extensionId,
        recordIds
          ? { connection, recordIds, idempotencyKey: key }
          : { connection, status, idempotencyKey: key },
      ),
    onSuccess: () => {
      setSelected([]);
      return client.invalidateQueries({
        queryKey: extensionExportQueryKeys.all,
      });
    },
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
      {page.error && <Alert severity="error">{errorMessage(page.error)}</Alert>}
      {retry.error && (
        <Alert severity="error">{errorMessage(retry.error)}</Alert>
      )}
      {!!retry.data?.job_errors.length && (
        <Alert severity="warning">
          {t('extensionExports.retryJobErrors', {
            messages: retry.data.job_errors
              .map((error) => error.message)
              .join('; '),
          })}
        </Alert>
      )}
      {rows.length === 0 ? (
        <EmptyState
          description={t('extensionExports.nothingExportedDescription')}
          icon={ExportIcon}
          title={t('extensionExports.nothingExported')}
        />
      ) : (
        <Box sx={{ overflowX: 'auto' }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('extensionExports.connection')}</TableCell>
                {exportStatuses.map((item) => (
                  <TableCell align="right" key={item}>
                    {t(`extensionExports.status.${item}`)}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {rows.map((row) => (
                <TableRow key={row.connection ?? allConnections}>
                  <TableCell>
                    {row.connection ?? t('extensionExports.defaultConnection')}
                  </TableCell>
                  {exportStatuses.map((item) => (
                    <TableCell align="right" key={item}>
                      {row.counts[item] ?? 0}
                    </TableCell>
                  ))}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Box>
      )}
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ mt: 3, mb: 1 }}
      >
        <FormControl size="small" sx={{ minWidth: 200 }}>
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
        <FormControl size="small" sx={{ minWidth: 200 }}>
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
              onClick={() =>
                retry.mutate({ recordIds: selected, key: crypto.randomUUID() })
              }
            >
              {t('extensionExports.retrySelected', { count: selected.length })}
            </Button>
            <Button
              disabled={!mappings.length || retry.isPending}
              startIcon={<RotateCcwIcon />}
              onClick={() =>
                retry.mutate({ recordIds: undefined, key: crypto.randomUUID() })
              }
            >
              {t('extensionExports.retryAll', {
                status: t(`extensionExports.status.${status}`),
              })}
            </Button>
          </>
        )}
      </Stack>
      {mappings.length === 0 ? (
        <Typography color="text.secondary" variant="body2">
          {t('extensionExports.noRecordsInStatus')}
        </Typography>
      ) : (
        <Table size="small">
          <TableHead>
            <TableRow>
              {canManage && retryable && <TableCell padding="checkbox" />}
              <TableCell>{t('extensionExports.record')}</TableCell>
              <TableCell>{t('extensionExports.statusColumn')}</TableCell>
              <TableCell>{t('extensionExports.error')}</TableCell>
              <TableCell>{t('extensionExports.updated')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {mappings.map((mapping) => (
              <TableRow key={mapping.record_id}>
                {canManage && retryable && (
                  <TableCell padding="checkbox">
                    <Checkbox
                      checked={selected.includes(mapping.record_id)}
                      slotProps={{
                        input: {
                          'aria-label': t('extensionExports.selectRecord', {
                            record: mapping.record_id,
                          }),
                        },
                      }}
                      onChange={(event) =>
                        setSelected((current) =>
                          event.target.checked
                            ? [...current, mapping.record_id]
                            : current.filter((id) => id !== mapping.record_id),
                        )
                      }
                    />
                  </TableCell>
                )}
                <TableCell sx={{ overflowWrap: 'anywhere' }}>
                  <Link
                    params={{ recordId: mapping.record_id }}
                    to="/records/$recordId"
                  >
                    {mapping.external_id ?? mapping.record_id}
                  </Link>
                </TableCell>
                <TableCell>
                  <ExportStatusChip status={mapping.status} />
                </TableCell>
                <TableCell>
                  {/* The extension's own recorded push error, not a failed request. */}
                  {/* eslint-disable-next-line no-restricted-syntax */}
                  {[mapping.error?.code, mapping.error?.message]
                    .filter(Boolean)
                    .join(': ') || '—'}
                </TableCell>
                <TableCell>
                  <Timestamp value={mapping.updated_at} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {page.hasNextPage && (
        <Box sx={{ mt: 2 }}>
          <LoadMoreButton
            isLoading={page.isFetchingNextPage}
            onLoadMore={() => void page.fetchNextPage()}
          />
        </Box>
      )}
      {handlerFailures.length > 0 && (
        <Box sx={{ mt: 3 }}>
          <Typography variant="subtitle1">
            {t('extensionExports.handlerFailures')}
          </Typography>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('extensionExports.event')}</TableCell>
                <TableCell>{t('extensionExports.error')}</TableCell>
                <TableCell>{t('extensionExports.attempts')}</TableCell>
                <TableCell>{t('extensionExports.updated')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {handlerFailures.map((failure) => (
                <TableRow key={`${failure.event_id}:${failure.handler_id}`}>
                  <TableCell>
                    {failure.event_type} · {failure.handler_id}
                  </TableCell>
                  <TableCell>
                    {[failure.error_code, failure.error_message]
                      .filter(Boolean)
                      .join(': ')}
                  </TableCell>
                  <TableCell>{failure.attempts}</TableCell>
                  <TableCell>
                    <Timestamp value={failure.updated_at} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Box>
      )}
      {exports.map((destination) => (
        <Box key={destination.id} sx={{ mt: 3 }}>
          <ExtensionOutlet
            context={{ context_version: 1, export_id: destination.id }}
            extensionId={extensionId}
            outlet={exportMappingExtensionOutlet}
          />
        </Box>
      ))}
    </Paper>
  );
};
