import { Chip, IconButton, Stack, Typography } from '@mui/material';
import { DownloadIcon, Trash2Icon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { fileDownloadUrl } from './api';
import { UPLOADED_FILE_THUMBNAIL_SIZE } from './constants';
import { FileThumbnail } from './FileThumbnail';
import type { FileMetadata } from './schemas';

type Props = {
  file: FileMetadata;
  /** Offers removal; left out where the file cannot change. */
  onRemove?: () => void;
  removeDisabled?: boolean;
  showThumbnail: boolean;
};

/** A stored file with its processing status and download link. */
export const UploadedFileRow = ({
  file,
  onRemove,
  removeDisabled,
  showThumbnail,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
      {showThumbnail && (
        <FileThumbnail file={file} size={UPLOADED_FILE_THUMBNAIL_SIZE} />
      )}
      <Typography sx={{ flexGrow: 1 }}>{file.filename}</Typography>
      <Chip label={t(`files.status.${file.status}`)} size="small" />
      <IconButton
        aria-label={t('files.downloadFile', { filename: file.filename })}
        component="a"
        href={fileDownloadUrl(file.id)}
      >
        <DownloadIcon />
      </IconButton>
      {onRemove && (
        <IconButton
          aria-label={t('files.removeImage', { filename: file.filename })}
          color="error"
          disabled={removeDisabled}
          onClick={onRemove}
        >
          <Trash2Icon />
        </IconButton>
      )}
    </Stack>
  );
};
