import CloudUploadOutlinedIcon from '@mui/icons-material/CloudUploadOutlined';
import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { fileCardinalities } from './constants';
import { acceptedFileTypes } from './fileAcceptance';
import { PendingFileRow } from './PendingFileRow';
import type { FileMetadata } from './schemas';
import { UploadedFileRow } from './UploadedFileRow';
import { usePendingFileUploads } from './usePendingFileUploads';

type FileAttributeEditorProps = {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  entityId?: string;
  files: FileMetadata[];
};

const FileAttributeEditorContent = (props: FileAttributeEditorProps) => {
  const { attribute, disabled, entityId } = props;
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const uploads = usePendingFileUploads(props);
  const policy = attribute.file_policy;
  if (!policy) return null;
  const queueDisabled = !uploads.canUpload || !uploads.canQueueFile;

  return (
    <Stack spacing={1}>
      <Typography>{attributeLabel(attribute)}</Typography>
      {!entityId && (
        <Alert severity="info">{t('files.saveEntityBeforeUploading')}</Alert>
      )}
      <Box
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          uploads.add(event.dataTransfer.files);
        }}
        sx={{
          border: '1px dashed',
          borderColor: 'divider',
          borderRadius: 1,
          p: 2,
        }}
      >
        <input
          accept={acceptedFileTypes(policy)}
          disabled={queueDisabled}
          hidden
          multiple={policy.cardinality === fileCardinalities.many}
          onChange={(event) => {
            if (event.target.files) uploads.add(event.target.files);
            event.target.value = '';
          }}
          ref={input}
          type="file"
        />
        <Button
          disabled={queueDisabled}
          onClick={() => input.current?.click()}
          startIcon={<CloudUploadOutlinedIcon />}
        >
          {t('files.chooseOrDropFiles')}
        </Button>
        {uploads.pending.length > 0 && (
          <Button
            color="primary"
            disabled={!uploads.canUpload || !uploads.hasQueuedFiles}
            onClick={uploads.uploadPending}
            variant="contained"
          >
            {t('files.uploadCount', { count: uploads.pending.length })}
          </Button>
        )}
      </Box>
      {uploads.pending.map((item) => (
        <PendingFileRow
          disabled={disabled}
          item={item}
          key={item.id}
          onRetry={() => uploads.retry(item)}
        />
      ))}
      {uploads.uploaded.map((file) => (
        <UploadedFileRow
          file={file}
          key={file.id}
          showThumbnail={policy.image_only}
        />
      ))}
    </Stack>
  );
};

/** Editor for a file attribute; resets its queue when the target changes. */
export const FileAttributeEditor = (props: FileAttributeEditorProps) => (
  <FileAttributeEditorContent
    key={`${props.entityId ?? ''}:${props.attribute.code}:${props.contextId ?? ''}`}
    {...props}
  />
);
