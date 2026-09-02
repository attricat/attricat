import { Box, CircularProgress, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { fileDownloadUrl, getFileMetadata } from './api';
import { fileQueryKeys } from './query-keys';
import type { FileMetadata } from './schemas';

type ThumbnailFile = Pick<FileMetadata, 'id' | 'filename'>;

const pollingStatuses = new Set(['uploading', 'queued', 'processing']);

export const FileThumbnail = ({
  file,
  size,
}: {
  file: ThumbnailFile;
  size: number;
}) => {
  const [loaded, setLoaded] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const metadata = useQuery({
    queryKey: fileQueryKeys.metadata(file.id),
    queryFn: () => getFileMetadata(file.id),
    refetchInterval: (query) =>
      pollingStatuses.has(query.state.data?.status ?? '') ? 1_000 : false,
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

  useEffect(() => {
    setLoaded(false);
    setAttempt(0);
  }, [source]);

  const retry = () => {
    window.setTimeout(() => setAttempt((value) => value + 1), 1_000);
  };

  const unavailable =
    currentFile?.status === 'failed' || currentFile?.status === 'deleted';

  return (
    <Box
      aria-busy={!loaded && !unavailable}
      aria-label={`Thumbnail for ${file.filename}`}
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
          onLoad={() => setLoaded(true)}
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
          {unavailable ? (
            <Typography color="text.secondary" variant="caption">
              Unavailable
            </Typography>
          ) : (
            <CircularProgress aria-label="Thumbnail is processing" size={20} />
          )}
        </Box>
      )}
    </Box>
  );
};
