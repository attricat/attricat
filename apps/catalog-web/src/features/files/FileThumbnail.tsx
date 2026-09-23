import { Box, CircularProgress, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { fileDownloadUrl, getFileMetadata } from './api';
import { fileQueryKeys } from './query-keys';
import type { FileMetadata } from './schemas';

type ThumbnailFile = Pick<FileMetadata, 'id' | 'filename'>;

type ThumbnailPreviewProps = {
  filename: string;
  size: number;
  source?: string;
  unavailable: boolean;
};

const pollingStatuses = new Set(['uploading', 'queued', 'processing']);
const thumbnailPollInterval = 1_000;
const maxThumbnailRetries = 3;

// Explorer virtualizes rows, mounting and unmounting thumbnails as its virtual
// range changes. Remember completed sources so a remounted thumbnail does not
// show its loading treatment again.
const loadedThumbnailSources = new Set<string>();
const maxLoadedThumbnailSources = 256;

const rememberLoadedThumbnail = (source: string) => {
  loadedThumbnailSources.delete(source);
  loadedThumbnailSources.add(source);
  if (loadedThumbnailSources.size > maxLoadedThumbnailSources) {
    const oldest = loadedThumbnailSources.values().next().value;
    if (oldest) loadedThumbnailSources.delete(oldest);
  }
};

const ThumbnailPreviewContent = ({
  filename,
  size,
  source,
  unavailable,
}: ThumbnailPreviewProps) => {
  const { t } = useTranslation();
  const [loaded, setLoaded] = useState(() =>
    source ? loadedThumbnailSources.has(source) : false,
  );
  const [attempt, setAttempt] = useState(0);
  const [retryExhausted, setRetryExhausted] = useState(false);
  const retryTimer = useRef<number | undefined>(undefined);
  const isUnavailable = unavailable || retryExhausted;

  useEffect(
    () => () => {
      if (retryTimer.current !== undefined) {
        window.clearTimeout(retryTimer.current);
        retryTimer.current = undefined;
      }
    },
    [],
  );

  const retry = () => {
    if (retryTimer.current !== undefined || retryExhausted) return;

    if (attempt >= maxThumbnailRetries) {
      setRetryExhausted(true);
      return;
    }

    retryTimer.current = window.setTimeout(() => {
      retryTimer.current = undefined;
      setAttempt((value) => value + 1);
    }, thumbnailPollInterval);
  };

  return (
    <Box
      aria-busy={!loaded && !isUnavailable}
      aria-label={t('files.thumbnailFor', { filename })}
      role="img"
      sx={{
        bgcolor: 'action.hover',
        borderRadius: 1,
        height: size,
        overflow: 'hidden',
        position: 'relative',
        width: size,
      }}
    >
      {source && (
        <Box
          alt=""
          component="img"
          onError={retry}
          onLoad={() => {
            rememberLoadedThumbnail(source);
            setLoaded(true);
          }}
          src={`${source}${attempt ? `?retry=${attempt}` : ''}`}
          sx={{
            height: '100%',
            objectFit: 'cover',
            opacity: loaded ? 1 : 0,
            transition: 'opacity 200ms ease-in',
            width: '100%',
          }}
        />
      )}
      {!loaded && (
        <Box
          sx={{
            alignItems: 'center',
            display: 'flex',
            height: '100%',
            inset: 0,
            justifyContent: 'center',
            position: 'absolute',
            width: '100%',
          }}
        >
          {isUnavailable ? (
            <Typography color="text.secondary" variant="caption">
              {t('files.thumbnailUnavailable')}
            </Typography>
          ) : (
            <CircularProgress
              aria-label={t('files.thumbnailProcessing')}
              enableTrackSlot
              size={20}
            />
          )}
        </Box>
      )}
    </Box>
  );
};

export const ThumbnailPreview = (props: ThumbnailPreviewProps) => (
  <ThumbnailPreviewContent key={props.source ?? 'none'} {...props} />
);

export const FileThumbnail = ({
  file,
  size,
}: {
  file: ThumbnailFile;
  size: number;
}) => {
  const metadata = useQuery({
    queryKey: fileQueryKeys.metadata(file.id),
    queryFn: () => getFileMetadata(file.id),
    refetchInterval: (query) =>
      pollingStatuses.has(query.state.data?.status ?? '')
        ? thumbnailPollInterval
        : false,
    refetchIntervalInBackground: true,
  });
  const currentFile = metadata.data;
  const thumbnail = currentFile?.variants.find(
    (variant) => variant.kind === 'thumbnail',
  );
  const source =
    currentFile?.status === 'ready'
      ? fileDownloadUrl(currentFile.id, thumbnail?.kind)
      : undefined;
  const unavailable =
    currentFile?.status === 'failed' || currentFile?.status === 'deleted';

  return (
    <ThumbnailPreview
      filename={file.filename}
      size={size}
      source={source}
      unavailable={unavailable}
    />
  );
};
