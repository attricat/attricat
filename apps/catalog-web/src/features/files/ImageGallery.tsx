import { Box, ButtonBase, Pagination, Stack, Typography } from '@mui/material';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { GALLERY_PAGE_SIZE, GALLERY_THUMBNAIL_SIZE } from './constants';
import { FileThumbnail } from './FileThumbnail';
import { ImagePreviewDialog } from './ImagePreviewDialog';

export type GalleryFile = { id: string; filename: string };

/** Presentation only: editing actions belong to the caller, never the viewer. */
export const ImageGallery = ({
  files,
  renderFilePanel,
  renderActions,
  renderItem = ({ file, children }) => (
    <Stack key={file.id} spacing={1} sx={{ minWidth: 0 }}>
      {children}
    </Stack>
  ),
}: {
  files: readonly GalleryFile[];
  renderFilePanel?: (fileId: string) => ReactNode;
  renderActions?: (file: GalleryFile, index: number) => ReactNode;
  /** Wraps each tile, e.g. to make it sortable; must return a keyed element. */
  renderItem?: (item: {
    children: ReactNode;
    file: GalleryFile;
    index: number;
  }) => ReactNode;
}) => {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  if (selectedId && !files.some((file) => file.id === selectedId)) {
    setSelectedId(null);
  }
  const pageCount = Math.max(1, Math.ceil(files.length / GALLERY_PAGE_SIZE));
  const currentPage = Math.min(page, pageCount);
  const start = (currentPage - 1) * GALLERY_PAGE_SIZE;
  return (
    <Stack spacing={2}>
      {!files.length && (
        <Typography color="text.secondary">{t('files.noImages')}</Typography>
      )}
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: `repeat(auto-fill, minmax(${GALLERY_THUMBNAIL_SIZE}px, 1fr))`,
          gap: 2,
        }}
      >
        {files.slice(start, start + GALLERY_PAGE_SIZE).map((file, index) =>
          renderItem({
            file,
            index: start + index,
            children: (
              <>
                <ButtonBase
                  aria-label={t('files.previewImage', {
                    filename: file.filename,
                  })}
                  onClick={() => setSelectedId(file.id)}
                  sx={{
                    alignSelf: 'flex-start',
                    borderRadius: 1,
                    '&.Mui-focusVisible': {
                      outline: '2px solid',
                      outlineColor: 'primary.main',
                      outlineOffset: 2,
                    },
                  }}
                >
                  <FileThumbnail file={file} size={GALLERY_THUMBNAIL_SIZE} />
                </ButtonBase>
                <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>
                  {file.filename}
                </Typography>
                {renderActions?.(file, start + index)}
                {renderFilePanel?.(file.id)}
              </>
            ),
          }),
        )}
      </Box>
      {pageCount > 1 && (
        <Pagination
          aria-label={t('files.galleryPages')}
          count={pageCount}
          page={currentPage}
          onChange={(_, next) => setPage(next)}
        />
      )}
      <ImagePreviewDialog
        files={files}
        selectedId={selectedId}
        onSelect={setSelectedId}
        onClose={() => setSelectedId(null)}
      />
    </Stack>
  );
};
