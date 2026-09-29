import {
  Box,
  Chip,
  IconButton,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material';
import { RotateCcwIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { PendingFile } from './usePendingFileUploads';

type Props = {
  disabled: boolean;
  item: PendingFile;
  onRetry: () => void;
};

/** A queued or uploading file with its progress or failure. */
export const PendingFileRow = ({ disabled, item, onRetry }: Props) => {
  const { t } = useTranslation();
  return (
    <Box>
      <Stack direction="row" spacing={1}>
        <Typography>{item.file.name}</Typography>
        {item.error ? (
          <>
            <Chip color="error" label={t('files.failed')} size="small" />
            <IconButton
              aria-label={t('files.retryFile', { filename: item.file.name })}
              disabled={disabled}
              onClick={onRetry}
            >
              <RotateCcwIcon />
            </IconButton>
          </>
        ) : (
          <Chip
            label={
              item.progress
                ? t('files.uploadProgress', { progress: item.progress })
                : t('files.ready')
            }
            size="small"
          />
        )}
      </Stack>
      {item.progress > 0 && (
        <LinearProgress value={item.progress} variant="determinate" />
      )}
      {item.error && (
        <Typography color="error" variant="body2">
          {item.error}
        </Typography>
      )}
    </Box>
  );
};
