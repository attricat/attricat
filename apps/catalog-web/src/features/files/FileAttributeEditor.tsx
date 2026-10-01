import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import { CloudUploadIcon } from 'lucide-react';
import { useId, useRef } from 'react';
import { ImageGalleryEditor } from './ImageGalleryEditor';
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
  error?: string;
  helperText?: string;
};

const FileAttributeEditorContent = (props: FileAttributeEditorProps) => {
  const { attribute, entityId, error, helperText } = props;
  const disabled = props.disabled || attribute.readonly === true;
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const uploads = usePendingFileUploads({ ...props, disabled });
  const labelId = useId();
  const helpId = useId();
  const policy = attribute.file_policy;
  if (!policy) return null;
  const queueDisabled = !uploads.canUpload || !uploads.canQueueFile;

  return (
    <Stack
      spacing={1}
      role="group"
      aria-labelledby={labelId}
      aria-describedby={helpId}
    >
      <Typography id={labelId}>{attributeLabel(attribute)}</Typography>
      <Typography id={helpId} variant="body2" color="text.secondary">
        {helperText ?? t('files.savedImmediately')}
      </Typography>
      {helperText && entityId && !disabled && (
        <Typography variant="body2" color="text.secondary">
          {t('files.savedImmediately')}
        </Typography>
      )}
      {error && <Alert severity="error">{error}</Alert>}
      {uploads.errors.map((message, index) => (
        <Alert severity="error" key={index}>
          {message}
        </Alert>
      ))}
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
          startIcon={<CloudUploadIcon />}
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
          disabled={disabled || uploads.busy}
          item={item}
          key={item.id}
          onRetry={() => uploads.retry(item)}
          onRemove={() => uploads.removePending(item.id)}
        />
      ))}
      {policy.image_only ? (
        <ImageGalleryEditor
          files={uploads.uploaded}
          disabled={!uploads.canUpload}
          ordered={policy.ordered}
          onChange={uploads.changeReferences}
        />
      ) : (
        uploads.uploaded.map((file) => (
          <UploadedFileRow file={file} key={file.id} showThumbnail={false} />
        ))
      )}
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
