import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { EyeIcon, RotateCcwIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { ExportIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { useCan } from '../auth/useCan';
import { recordExportStatus, retryExports } from './api';
import { retryableExportStatuses } from './constants';
import { ExportPreviewDialog } from './ExportPreviewDialog';
import { ExportStatusChip } from './ExportStatusChip';
import { extensionExportQueryKeys } from './queryKeys';
import type {
  ConnectionLabel,
  ExportDestination,
  ExportMapping,
} from './schemas';

type Row = {
  key: string;
  connection: ConnectionLabel | undefined;
  destination: ExportDestination | undefined;
  extensionId: string;
  mapping: ExportMapping | undefined;
};

/**
 * One row per mapping, matched with its destination through the
 * connection's kind, plus enabled destinations the record has none at.
 */
const exportRows = (
  statuses: ExportMapping[],
  destinations: ExportDestination[],
  connections: ConnectionLabel[],
): Row[] => {
  const rows = statuses.map((mapping) => {
    const connection = connections.find(
      (label) =>
        label.extension_id === mapping.extension_id &&
        label.connection === mapping.connection,
    );
    const candidates = destinations.filter(
      (destination) => destination.extension_id === mapping.extension_id,
    );
    // A connection's mapping belongs only to an export of its kind; the
    // default destination's to an export without one, else any export.
    const destination =
      mapping.connection === null
        ? (candidates.find(
            (destination) => destination.connection_kind === null,
          ) ?? candidates[0])
        : candidates.find(
            (destination) =>
              connection !== undefined &&
              destination.connection_kind === connection.kind,
          );
    return {
      key: `mapping:${mapping.extension_id}:${mapping.connection ?? ''}`,
      connection,
      destination,
      extensionId: mapping.extension_id,
      mapping,
    };
  });
  const unused = destinations
    .filter(
      (destination) =>
        !rows.some(
          (row) =>
            row.destination?.extension_id === destination.extension_id &&
            row.destination.export_id === destination.export_id,
        ),
    )
    .map((destination) => ({
      key: `export:${destination.extension_id}:${destination.export_id}`,
      connection: undefined,
      destination,
      extensionId: destination.extension_id,
      mapping: undefined,
    }));
  return [...rows, ...unused];
};

/**
 * Where a record is exported: its status at each export extension's
 * destination, the destination's error, and retry and preview actions.
 */
export const RecordExportStatusPanel = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const canManage = useCan('extensions_manage');
  // A preview runs with the extension's catalog access, so it needs
  // workspace-wide read access.
  const canPreview = useCan('records_read');
  const [preview, setPreview] = useState<{
    destination: ExportDestination;
    connection: string | null;
  }>();
  const status = useQuery({
    queryKey: extensionExportQueryKeys.record(recordId),
    queryFn: () => recordExportStatus(recordId),
  });
  const retry = useMutation({
    mutationFn: (mapping: ExportMapping) =>
      retryExports(mapping.extension_id, {
        connection: mapping.connection,
        recordIds: [recordId],
      }),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: extensionExportQueryKeys.all }),
  });
  if (status.isPending) {
    return <CircularProgress aria-label={t('extensionExports.loading')} />;
  }
  if (status.error) {
    return <Alert severity="error">{status.error.message}</Alert>;
  }
  const rows = exportRows(
    status.data.statuses,
    status.data.destinations,
    status.data.connections,
  );
  if (!rows.length) {
    return (
      <EmptyState
        description={t('extensionExports.noDestinationsDescription')}
        icon={ExportIcon}
        title={t('extensionExports.noDestinations')}
      />
    );
  }
  return (
    <Box>
      {retry.error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          {retry.error.message}
        </Alert>
      )}
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('extensionExports.destination')}</TableCell>
            <TableCell>{t('extensionExports.statusColumn')}</TableCell>
            <TableCell>{t('extensionExports.externalId')}</TableCell>
            <TableCell>{t('extensionExports.lastSynced')}</TableCell>
            <TableCell align="right">{t('extensionExports.actions')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map(
            ({ connection, destination, extensionId, key, mapping }) => (
              <TableRow key={key}>
                <TableCell>
                  <Typography variant="body2">
                    {destination?.title ?? extensionId}
                  </Typography>
                  <Typography color="text.secondary" variant="caption">
                    {connection?.display_name ??
                      mapping?.connection ??
                      t('extensionExports.defaultConnection')}
                  </Typography>
                </TableCell>
                <TableCell>
                  <Stack spacing={0.5} sx={{ alignItems: 'flex-start' }}>
                    {mapping ? (
                      <ExportStatusChip status={mapping.status} />
                    ) : (
                      <Typography color="text.secondary" variant="body2">
                        {t('extensionExports.notExported')}
                      </Typography>
                    )}
                    {mapping?.error && (
                      <Typography color="error" variant="caption">
                        {[mapping.error.code, mapping.error.message]
                          .filter(Boolean)
                          .join(': ')}
                      </Typography>
                    )}
                  </Stack>
                </TableCell>
                <TableCell sx={{ overflowWrap: 'anywhere' }}>
                  {mapping?.external_id ?? '—'}
                </TableCell>
                <TableCell>
                  {mapping?.synced_at ? (
                    <Timestamp value={mapping.synced_at} />
                  ) : (
                    '—'
                  )}
                </TableCell>
                <TableCell align="right">
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ justifyContent: 'flex-end' }}
                  >
                    {canPreview && destination?.preview && (
                      <Button
                        size="small"
                        startIcon={<EyeIcon />}
                        onClick={() =>
                          setPreview({
                            destination,
                            connection: mapping?.connection ?? null,
                          })
                        }
                      >
                        {t('extensionExports.preview')}
                      </Button>
                    )}
                    {canManage &&
                      mapping &&
                      retryableExportStatuses.includes(mapping.status) && (
                        <Button
                          disabled={retry.isPending}
                          size="small"
                          startIcon={<RotateCcwIcon />}
                          onClick={() => retry.mutate(mapping)}
                        >
                          {t('extensionExports.retry')}
                        </Button>
                      )}
                  </Stack>
                </TableCell>
              </TableRow>
            ),
          )}
        </TableBody>
      </Table>
      {preview && (
        <ExportPreviewDialog
          connection={preview.connection}
          destination={preview.destination}
          recordId={recordId}
          onClose={() => setPreview(undefined)}
        />
      )}
    </Box>
  );
};
