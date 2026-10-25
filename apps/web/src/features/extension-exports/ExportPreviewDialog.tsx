import {
  Alert,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { JsonBlock } from '../../components/JsonValue';
import { previewExport } from './api';
import type { ExportDestination } from './schemas';
import { errorMessage } from '../../api/errors';

/**
 * The payload a destination would receive for one record, with the issues
 * it would reject. Nothing is pushed.
 */
export const ExportPreviewDialog = ({
  connection,
  destination,
  onClose,
  recordId,
}: {
  connection: string | null;
  destination: ExportDestination;
  onClose: () => void;
  recordId: string;
}) => {
  const { t } = useTranslation();
  const preview = useQuery({
    queryKey: [
      'extension-exports',
      'preview',
      destination.extension_id,
      destination.export_id,
      recordId,
      connection,
    ],
    queryFn: () =>
      previewExport(
        destination.extension_id,
        destination.export_id,
        recordId,
        connection,
      ),
    gcTime: 0,
  });
  const issues = preview.data?.issues ?? [];
  return (
    <Dialog fullWidth maxWidth="md" open onClose={onClose}>
      <DialogTitle>
        {t('extensionExports.previewTitle', { destination: destination.title })}
      </DialogTitle>
      <DialogContent>
        {preview.isPending && (
          <CircularProgress aria-label={t('extensionExports.previewLoading')} />
        )}
        {preview.error && (
          <Alert severity="error">{errorMessage(preview.error)}</Alert>
        )}
        {preview.data && (
          <Stack spacing={2}>
            {issues.length ? (
              <Alert severity="warning">
                {t('extensionExports.previewIssues', { count: issues.length })}
                <ul>
                  {issues.map((issue) => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              </Alert>
            ) : (
              <Alert severity="success">
                {t('extensionExports.previewValid')}
              </Alert>
            )}
            <JsonBlock value={preview.data.payload ?? preview.data} />
          </Stack>
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('extensionExports.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};
