import {
  Box,
  Chip,
  IconButton,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material';
import { RotateCcwIcon, XIcon } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SUPPORTED_IMAGE_MIME_TYPES,
  UPLOADED_FILE_THUMBNAIL_SIZE,
} from './constants';
import type { PendingFile } from './usePendingFileUploads';

const PendingPreview = ({ file }: { file: File }) => {
  const image = useRef<HTMLImageElement>(null);
  useEffect(() => {
    const element = image.current;
    if (!element || !SUPPORTED_IMAGE_MIME_TYPES.has(file.type)) return;
    const url = URL.createObjectURL(file);
    element.src = url;
    return () => {
      element.removeAttribute('src');
      URL.revokeObjectURL(url);
    };
  }, [file]);
  return SUPPORTED_IMAGE_MIME_TYPES.has(file.type) ? (
    <Box
      component="img"
      ref={image}
      alt=""
      sx={{
        width: UPLOADED_FILE_THUMBNAIL_SIZE,
        height: UPLOADED_FILE_THUMBNAIL_SIZE,
        objectFit: 'cover',
        borderRadius: 1,
      }}
    />
  ) : null;
};

/** A queued or uploading file with its progress or failure. */
export const PendingFileRow = ({
  disabled,
  item,
  onRetry,
  onRemove,
}: {
  disabled: boolean;
  item: PendingFile;
  onRetry: () => void;
  onRemove: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <Box>
      <Stack direction="row" spacing={1}>
        <PendingPreview file={item.file} />
        <Typography sx={{ overflowWrap: 'anywhere' }}>
          {item.file.name}
        </Typography>
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
        <IconButton
          aria-label={t('files.removeQueuedFile', { filename: item.file.name })}
          disabled={disabled || item.progress > 0}
          onClick={onRemove}
        >
          <XIcon />
        </IconButton>
      </Stack>
      {item.progress > 0 && (
        <LinearProgress
          aria-label={t('files.uploadProgress', { progress: item.progress })}
          value={item.progress}
          variant="determinate"
        />
      )}
      {item.error && (
        <Typography role="alert" color="error" variant="body2">
          {item.error}
        </Typography>
      )}
    </Box>
  );
};
