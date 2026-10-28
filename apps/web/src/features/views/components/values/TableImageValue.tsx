import { Box } from '@mui/material';
import { ImageIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../../../components/iconSizes';
import { fileDownloadUrl } from '../../../files/api';
import { fileStatuses, THUMBNAIL_VARIANT_KIND } from '../../../files/constants';
import { ThumbnailPreview } from '../../../files/FileThumbnail';
import { fileMetadataSchema } from '../../../files/schemas';
import { TABLE_IMAGE_SIZE } from '../../constants';

// Built once: creating a schema costs more than parsing every table cell.
const fileListSchema = fileMetadataSchema.array();

/** An empty thumbnail tile, so rows without an image keep the column aligned. */
const MissingImage = () => {
  const { t } = useTranslation();
  return (
    <Box
      aria-label={t('files.noImage')}
      role="img"
      sx={{
        alignItems: 'center',
        bgcolor: 'action.hover',
        borderRadius: 1,
        color: 'text.disabled',
        display: 'flex',
        height: TABLE_IMAGE_SIZE,
        justifyContent: 'center',
        width: TABLE_IMAGE_SIZE,
      }}
    >
      <ImageIcon aria-hidden size={smallIconSize} />
    </Box>
  );
};

/** Thumbnail for `attricat.table_image@1`: the first file of an image-only attribute. */
export const TableImageValue = ({ value }: { value: unknown }) => {
  const singleFile = fileMetadataSchema.safeParse(value);
  const files = singleFile.success
    ? undefined
    : fileListSchema.safeParse(value);
  const file = singleFile.success
    ? singleFile.data
    : files?.success
      ? files.data[0]
      : undefined;

  if (!file) return <MissingImage />;

  const thumbnail = file.variants.find(
    (variant) => variant.kind === THUMBNAIL_VARIANT_KIND,
  );
  // Never fall back to the original: it can be a full-resolution upload.
  return (
    <ThumbnailPreview
      filename={file.filename}
      size={TABLE_IMAGE_SIZE}
      source={
        file.status === fileStatuses.ready && thumbnail
          ? fileDownloadUrl(file.id, thumbnail.kind)
          : undefined
      }
      unavailable={
        (file.status === fileStatuses.ready && !thumbnail) ||
        file.status === fileStatuses.failed ||
        file.status === fileStatuses.deleted
      }
    />
  );
};
