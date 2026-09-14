import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { fileDownloadUrl } from '../files/api';
import { ThumbnailPreview } from '../files/FileThumbnail';
import { fileMetadataSchema } from '../files/schemas';

const imageCellSize = 48;

export const ImageTableCell = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const singleFile = fileMetadataSchema.safeParse(value);
  const files = fileMetadataSchema.array().safeParse(value);
  const file = singleFile.success
    ? singleFile.data
    : files.success
      ? files.data[0]
      : undefined;

  if (!file)
    return (
      <Typography color="text.secondary" variant="body2">
        {t('views.notSet')}
      </Typography>
    );

  const thumbnail = file.variants.find((variant) => variant.kind === 'thumbnail');
  return (
    <ThumbnailPreview
      filename={file.filename}
      size={imageCellSize}
      source={
        file.status === 'ready'
          ? fileDownloadUrl(file.id, thumbnail?.kind)
          : undefined
      }
      unavailable={file.status === 'failed' || file.status === 'deleted'}
    />
  );
};
