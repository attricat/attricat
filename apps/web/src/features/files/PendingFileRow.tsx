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
import { FileCard } from './FileCard';
import type { PendingFile } from './usePendingFileUploads';

/** A picked image; without a size, a square filling its parent. */
const PendingPreview = ({ file, size }: { file: File; size?: number }) => {
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
        aspectRatio: '1',
        borderRadius: 1,
        display: 'block',
        height: size,
        objectFit: 'cover',
        width: size ?? '100%',
      }}
    />
  ) : null;
};

type PendingFileProps = {
  disabled: boolean;
  item: PendingFile;
  onRetry: () => void;
  onRemove: () => void;
};

/** A queued file's state, with retry after a failure and removal. */
const PendingFileActions = ({
  disabled,
  item,
  onRetry,
  onRemove,
}: PendingFileProps) => {
  const { t } = useTranslation();
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
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
        color="error"
        disabled={disabled || item.progress > 0}
        onClick={onRemove}
        sx={{ ml: 'auto' }}
      >
        <XIcon />
      </IconButton>
    </Stack>
  );
};

const PendingProgress = ({ item }: { item: PendingFile }) => {
  const { t } = useTranslation();
  return item.progress > 0 ? (
    <LinearProgress
      aria-label={t('files.uploadProgress', { progress: item.progress })}
      value={item.progress}
      variant="determinate"
    />
  ) : null;
};

const PendingError = ({ item }: { item: PendingFile }) =>
  item.error ? (
    <Typography role="alert" color="error" variant="body2">
      {item.error}
    </Typography>
  ) : null;

/** A queued or uploading file with its progress or failure. */
export const PendingFileRow = (props: PendingFileProps) => (
  <Box>
    <Stack direction="row" spacing={1}>
      <PendingPreview
        file={props.item.file}
        size={UPLOADED_FILE_THUMBNAIL_SIZE}
      />
      <Typography sx={{ overflowWrap: 'anywhere' }}>
        {props.item.file.name}
      </Typography>
      <PendingFileActions {...props} />
    </Stack>
    <PendingProgress item={props.item} />
    <PendingError item={props.item} />
  </Box>
);

/**
 * A queued or uploading image on the same card as the attached images beside
 * it, so a picked image reads as part of the field.
 */
export const PendingImageTile = (props: PendingFileProps) => (
  <FileCard
    actions={<PendingFileActions {...props} />}
    filename={props.item.file.name}
    preview={
      <Box>
        <PendingPreview file={props.item.file} />
        <PendingProgress item={props.item} />
      </Box>
    }
  >
    <PendingError item={props.item} />
  </FileCard>
);
