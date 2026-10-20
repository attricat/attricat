import { fileDownloadUrl } from '../../../files/api';
import { fileStatuses, THUMBNAIL_VARIANT_KIND } from '../../../files/constants';
import { ThumbnailPreview } from '../../../files/FileThumbnail';
import { fileMetadataSchema } from '../../../files/schemas';
import { TABLE_IMAGE_SIZE } from '../../constants';
import { NotSetValue } from './NotSetValue';

// Built once: creating a schema costs more than parsing every table cell.
const fileListSchema = fileMetadataSchema.array();

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

  if (!file) return <NotSetValue />;

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
