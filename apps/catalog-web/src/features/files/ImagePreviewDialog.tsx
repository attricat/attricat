import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  DownloadIcon,
  ShrinkIcon,
  ZoomInIcon,
  ZoomOutIcon,
} from 'lucide-react';
import { fileDownloadUrl, getFileMetadata } from './api';
import {
  DISPLAY_VARIANT_KIND,
  fileStatuses,
  IMAGE_MAX_ZOOM,
  IMAGE_MIN_ZOOM,
  IMAGE_ZOOM_STEP,
  THUMBNAIL_POLL_INTERVAL,
  THUMBNAIL_POLLING_STATUSES,
} from './constants';
import { fileQueryKeys } from './queryKeys';
import type { GalleryFile } from './ImageGallery';

const ImagePreview = ({ file }: { file: GalleryFile }) => {
  const { t } = useTranslation();
  const [zoom, setZoom] = useState(IMAGE_MIN_ZOOM);
  const [failed, setFailed] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const viewport = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const metadata = useQuery({
    queryKey: fileQueryKeys.metadata(file.id),
    queryFn: () => getFileMetadata(file.id),
    retry: false,
    refetchInterval: (query) =>
      !query.state.error &&
      THUMBNAIL_POLLING_STATUSES.has(query.state.data?.status ?? '')
        ? THUMBNAIL_POLL_INTERVAL
        : false,
  });
  const variant = metadata.data?.variants.find(
    (item) => item.kind === DISPLAY_VARIANT_KIND,
  );
  const ready = metadata.data?.status === fileStatuses.ready;
  const unavailable =
    metadata.isError ||
    failed ||
    (ready && !variant) ||
    metadata.data?.status === fileStatuses.failed ||
    metadata.data?.status === fileStatuses.deleted;
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const measure = () =>
      setSize({ width: element.clientWidth, height: element.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const width = variant?.width ?? size.width;
  const height = variant?.height ?? size.height;
  const fit =
    width && height ? Math.min(size.width / width, size.height / height, 1) : 1;
  const changeZoom = (next: number) => {
    setZoom(Math.max(IMAGE_MIN_ZOOM, Math.min(IMAGE_MAX_ZOOM, next)));
    viewport.current?.scrollTo?.({ left: 0, top: 0 });
  };
  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
        <Button
          disabled={!loaded || unavailable || zoom <= IMAGE_MIN_ZOOM}
          onClick={() => changeZoom(zoom - IMAGE_ZOOM_STEP)}
          startIcon={<ZoomOutIcon />}
        >
          {t('files.zoomOut')}
        </Button>
        <Button
          disabled={!loaded || unavailable || zoom >= IMAGE_MAX_ZOOM}
          onClick={() => changeZoom(zoom + IMAGE_ZOOM_STEP)}
          startIcon={<ZoomInIcon />}
        >
          {t('files.zoomIn')}
        </Button>
        <Button
          disabled={!loaded || unavailable}
          onClick={() => changeZoom(IMAGE_MIN_ZOOM)}
          startIcon={<ShrinkIcon />}
        >
          {t('files.fitImage')}
        </Button>
        <Typography aria-live="polite" sx={{ alignSelf: 'center' }}>
          {t('files.zoomLevel', { percent: zoom * 100 })}
        </Typography>
      </Stack>
      <Box
        ref={viewport}
        role="region"
        aria-label={t('files.imageViewport')}
        tabIndex={0}
        sx={{ height: '60vh', overflow: 'auto', bgcolor: 'background.default' }}
      >
        {unavailable ? (
          <Alert severity="warning">{t('files.imageUnavailable')}</Alert>
        ) : (
          <>
            {!loaded && (
              <CircularProgress aria-label={t('files.imageLoading')} />
            )}
            {ready && variant && (
              <Box
                component="img"
                alt={file.filename}
                src={fileDownloadUrl(file.id, DISPLAY_VARIANT_KIND)}
                onLoad={() => setLoaded(true)}
                onError={() => setFailed(true)}
                sx={{
                  display: 'block',
                  mx: 'auto',
                  width: width * fit * zoom,
                  height: height * fit * zoom,
                  maxWidth: 'none',
                  visibility: loaded ? 'visible' : 'hidden',
                }}
              />
            )}
          </>
        )}
      </Box>
      {ready && !metadata.isError && (
        <Button
          component="a"
          href={fileDownloadUrl(file.id)}
          startIcon={<DownloadIcon />}
        >
          {t('files.downloadFile', { filename: file.filename })}
        </Button>
      )}
    </Stack>
  );
};

export const ImagePreviewDialog = ({
  files,
  selectedId,
  onSelect,
  onClose,
}: {
  files: readonly GalleryFile[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  const title = useRef<HTMLHeadingElement>(null);
  const select = (id: string) => {
    // A navigation button may become disabled at the end of the gallery.
    // Keep keyboard focus inside the dialog (including Escape handling).
    title.current?.focus();
    onSelect(id);
  };
  const index = files.findIndex((file) => file.id === selectedId);
  const file = files[index];
  return (
    <Dialog
      open={Boolean(file)}
      onClose={onClose}
      fullWidth
      maxWidth="lg"
      aria-labelledby={titleId}
    >
      {file && (
        <>
          <DialogTitle id={titleId} ref={title} tabIndex={-1}>
            {file.filename}
          </DialogTitle>
          <DialogContent>
            <ImagePreview key={file.id} file={file} />
          </DialogContent>
          <DialogActions sx={{ flexWrap: 'wrap' }}>
            <Button
              disabled={index === 0}
              onClick={() => select(files[index - 1].id)}
              startIcon={<ChevronLeftIcon />}
            >
              {t('files.previousImage')}
            </Button>
            <Typography aria-live="polite">
              {t('files.imagePosition', {
                position: index + 1,
                count: files.length,
              })}
            </Typography>
            <Button
              disabled={index === files.length - 1}
              endIcon={<ChevronRightIcon />}
              onClick={() => select(files[index + 1].id)}
            >
              {t('files.nextImage')}
            </Button>
            <Button onClick={onClose}>{t('files.closePreview')}</Button>
          </DialogActions>
        </>
      )}
    </Dialog>
  );
};
