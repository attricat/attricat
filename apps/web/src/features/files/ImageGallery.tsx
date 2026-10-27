import { Box, ButtonBase, Pagination, Stack, Typography } from '@mui/material';
import { Fragment, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { GALLERY_CARD_MIN_WIDTH, GALLERY_PAGE_SIZE } from './constants';
import { FileCard } from './FileCard';
import { FileThumbnail } from './FileThumbnail';
import { ImagePreviewDialog } from './ImagePreviewDialog';
import { focusRing } from '../../app/theme';

export type GalleryFile = { id: string; filename: string };

/** Presentation only: editing actions belong to the caller, never the viewer. */
export const ImageGallery = ({
  files,
  pendingTiles = [],
  renderFilePanel,
  renderActions,
  renderItem = ({ file, children }) => (
    <Fragment key={file.id}>{children}</Fragment>
  ),
}: {
  files: readonly GalleryFile[];
  /** Images picked but not attached yet, shown after the attached ones. */
  pendingTiles?: readonly ReactNode[];
  renderFilePanel?: (fileId: string) => ReactNode;
  renderActions?: (file: GalleryFile, index: number) => ReactNode;
  /** Wraps each card, e.g. to make it sortable; must return a keyed element. */
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
  // Newly picked images join the last page, so show it.
  const [pendingCount, setPendingCount] = useState(pendingTiles.length);
  if (pendingTiles.length !== pendingCount) {
    if (pendingTiles.length > pendingCount) setPage(pageCount);
    setPendingCount(pendingTiles.length);
  }
  const currentPage = Math.min(page, pageCount);
  const start = (currentPage - 1) * GALLERY_PAGE_SIZE;
  return (
    <Stack spacing={2}>
      {!files.length && !pendingTiles.length && (
        <Typography color="text.secondary">{t('files.noImages')}</Typography>
      )}
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: `repeat(auto-fill, minmax(${GALLERY_CARD_MIN_WIDTH}px, 1fr))`,
          gap: 2,
        }}
      >
        {files.slice(start, start + GALLERY_PAGE_SIZE).map((file, index) =>
          renderItem({
            file,
            index: start + index,
            children: (
              <FileCard
                actions={renderActions?.(file, start + index)}
                filename={file.filename}
                preview={
                  <ButtonBase
                    aria-label={t('files.previewImage', {
                      filename: file.filename,
                    })}
                    onClick={() => setSelectedId(file.id)}
                    sx={{
                      borderRadius: 1,
                      display: 'block',
                      width: '100%',
                      '&.Mui-focusVisible': focusRing,
                    }}
                  >
                    <FileThumbnail file={file} />
                  </ButtonBase>
                }
              >
                {renderFilePanel?.(file.id)}
              </FileCard>
            ),
          }),
        )}
        {currentPage === pageCount && pendingTiles}
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
