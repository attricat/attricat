import { fileDownloadUrl } from '../../../files/api';
import { ThumbnailPreview } from '../../../files/FileThumbnail';
import { fileMetadataSchema } from '../../../files/schemas';
import { TABLE_IMAGE_SIZE } from '../../constants';
import { NotSetValue } from './NotSetValue';

/** Thumbnail for `catalog.table_image@1`: the first file of an image-only attribute. */
export const TableImageValue = ({ value }: { value: unknown }) => {
  const singleFile = fileMetadataSchema.safeParse(value);
  const files = fileMetadataSchema.array().safeParse(value);
  const file = singleFile.success
    ? singleFile.data
    : files.success
      ? files.data[0]
      : undefined;

  if (!file) return <NotSetValue />;

  const thumbnail = file.variants.find(
    (variant) => variant.kind === 'thumbnail',
  );
  return (
    <ThumbnailPreview
      filename={file.filename}
      size={TABLE_IMAGE_SIZE}
      source={
        file.status === 'ready'
          ? fileDownloadUrl(file.id, thumbnail?.kind)
          : undefined
      }
      unavailable={file.status === 'failed' || file.status === 'deleted'}
    />
  );
};
