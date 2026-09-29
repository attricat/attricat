import { Box, CircularProgress, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { fileDownloadUrl, getFileMetadata } from './api';
import {
  fileStatuses,
  MAX_LOADED_THUMBNAIL_SOURCES,
  MAX_THUMBNAIL_RETRIES,
  THUMBNAIL_FADE_IN_TRANSITION,
  THUMBNAIL_POLL_INTERVAL,
  THUMBNAIL_POLLING_STATUSES,
  THUMBNAIL_RETRY_QUERY_PARAMETER,
  THUMBNAIL_SPINNER_SIZE,
  THUMBNAIL_VARIANT_KIND,
} from './constants';
import { fileQueryKeys } from './queryKeys';
import type { FileMetadata } from './schemas';

type ThumbnailFile = Pick<FileMetadata, 'id' | 'filename'>;

type ThumbnailPreviewProps = {
  filename: string;
  size: number;
  source?: string;
  unavailable: boolean;
};

// Explorer virtualizes rows, mounting and unmounting thumbnails as its virtual
// range changes. Remember completed sources so a remounted thumbnail does not
// show its loading treatment again.
const loadedThumbnailSources = new Set<string>();

const rememberLoadedThumbnail = (source: string) => {
  loadedThumbnailSources.delete(source);
  loadedThumbnailSources.add(source);
  if (loadedThumbnailSources.size > MAX_LOADED_THUMBNAIL_SOURCES) {
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

    if (attempt >= MAX_THUMBNAIL_RETRIES) {
      setRetryExhausted(true);
      return;
    }

    retryTimer.current = window.setTimeout(() => {
      retryTimer.current = undefined;
      setAttempt((value) => value + 1);
    }, THUMBNAIL_POLL_INTERVAL);
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
          src={
            attempt
              ? `${source}?${THUMBNAIL_RETRY_QUERY_PARAMETER}=${attempt}`
              : source
          }
          sx={{
            height: '100%',
            objectFit: 'cover',
            opacity: loaded ? 1 : 0,
            transition: THUMBNAIL_FADE_IN_TRANSITION,
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
              size={THUMBNAIL_SPINNER_SIZE}
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
      THUMBNAIL_POLLING_STATUSES.has(query.state.data?.status ?? '')
        ? THUMBNAIL_POLL_INTERVAL
        : false,
    refetchIntervalInBackground: true,
  });
  const currentFile = metadata.data;
  const thumbnail = currentFile?.variants.find(
    (variant) => variant.kind === THUMBNAIL_VARIANT_KIND,
  );
  const source =
    currentFile?.status === fileStatuses.ready && thumbnail
      ? fileDownloadUrl(currentFile.id, thumbnail.kind)
      : undefined;
  const unavailable =
    (currentFile?.status === fileStatuses.ready && !thumbnail) ||
    currentFile?.status === fileStatuses.failed ||
    currentFile?.status === fileStatuses.deleted;

  return (
    <ThumbnailPreview
      filename={file.filename}
      size={size}
      source={source}
      unavailable={unavailable}
    />
  );
};
