import {
  Alert,
  Box,
  Button,
  FormControl,
  FormHelperText,
  FormLabel,
  IconButton,
  Stack,
  Tooltip,
} from '@mui/material';
import { CloudUploadIcon, InfoIcon } from 'lucide-react';
import { IconLabel } from '../../components/IconLabel';
import { compactIconSize } from '../../components/iconSizes';
import { useId, useRef, useState } from 'react';
import { ImageGalleryEditor } from './ImageGalleryEditor';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../records/api';
import { attributeLabel } from '../records/recordDisplay';
import { attributeCardinalities } from '../records/constants';
import { acceptedFileTypes } from './fileAcceptance';
import { fileAttributeIcon } from './fileAttributeIcon';
import { PendingFileRow, PendingImageTile } from './PendingFileRow';
import type { FileMetadata } from './schemas';
import { UploadedFileRow } from './UploadedFileRow';
import { usePendingFileUploads } from './usePendingFileUploads';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { FieldHint } from '../../components/FieldHint';

type FileAttributeEditorProps = {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  recordId?: string;
  files: FileMetadata[];
  error?: string;
  helperText?: string;
  onRecordUpdated?: (updatedAt: string) => void;
  required?: boolean;
};

const FileAttributeEditorContent = (props: FileAttributeEditorProps) => {
  const { attribute, recordId, error, helperText, required } = props;
  const disabled = props.disabled || attribute.readonly === true;
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const uploads = usePendingFileUploads({ ...props, disabled });
  const [removeFile, setRemoveFile] = useState<FileMetadata | null>(null);
  const helpId = useId();
  const errorId = useId();
  const policy = attribute.file_policy;
  if (!policy) return null;
  // The server takes an empty list, which would leave a required field empty.
  const lastRequiredFile = required === true && uploads.uploaded.length === 1;
  const queueDisabled = !uploads.canQueue || !uploads.canQueueFile;
  const pendingProps = (item: (typeof uploads.pending)[number]) => ({
    disabled: !uploads.canQueue,
    item,
    onRetry: () => uploads.retry(item),
    onRemove: () => uploads.removePending(item.id),
  });
  const savingHelp = uploads.deferred
    ? t('files.uploadedOnCreate')
    : !recordId
      ? t('files.saveRecordBeforeUploading')
      : !disabled && t('files.savedImmediately');

  return (
    // A fieldset names the drop zone, queue and gallery as one field, with
    // the same label, required mark and error colour as other fields.
    <FormControl
      aria-describedby={
        [helperText && helpId, error && errorId].filter(Boolean).join(' ') ||
        undefined
      }
      component="fieldset"
      disabled={disabled}
      error={Boolean(error)}
      fullWidth
      required={required}
    >
      {/* The legend must be the fieldset's first child to name it. */}
      <FormLabel component="legend" sx={{ mb: 0.5, p: 0 }}>
        <IconLabel icon={fileAttributeIcon(attribute)}>
          {attributeLabel(attribute)}
        </IconLabel>
      </FormLabel>
      <Stack spacing={1}>
        {(helperText || savingHelp) && (
          <Box sx={{ alignItems: 'center', display: 'flex', gap: 0.5 }}>
            {helperText && (
              <FormHelperText error={false} id={helpId} sx={{ m: 0 }}>
                <FieldHint>{helperText}</FieldHint>
              </FormHelperText>
            )}
            {savingHelp && (
              // Focusable so keyboard users can read it; it also describes
              // the button for assistive technology.
              <Tooltip describeChild enterTouchDelay={0} title={savingHelp}>
                <IconButton aria-label={t('files.savingHelp')} size="small">
                  <InfoIcon size={compactIconSize} />
                </IconButton>
              </Tooltip>
            )}
          </Box>
        )}
        {error && (
          <FormHelperText id={errorId} sx={{ m: 0 }}>
            {error}
          </FormHelperText>
        )}
        {uploads.errors.map((message, index) => (
          <Alert severity="error" key={index}>
            {message}
          </Alert>
        ))}
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
            multiple={policy.cardinality === attributeCardinalities.many}
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
        </Box>
        {policy.image_only ? (
          <ImageGalleryEditor
            files={uploads.uploaded}
            disabled={!uploads.canUpload}
            keepsLastFile={required}
            ordered={policy.ordered}
            // Picked images sit in the grid with the attached ones.
            pendingTiles={uploads.pending.map((item) => (
              <PendingImageTile key={item.id} {...pendingProps(item)} />
            ))}
            onChange={uploads.changeReferences}
          />
        ) : (
          <>
            {uploads.pending.map((item) => (
              <PendingFileRow key={item.id} {...pendingProps(item)} />
            ))}
            {uploads.uploaded.map((file) => (
              <UploadedFileRow
                file={file}
                key={file.id}
                onRemove={
                  disabled || !recordId || lastRequiredFile
                    ? undefined
                    : () => setRemoveFile(file)
                }
                removeDisabled={!uploads.canUpload}
                showThumbnail={false}
              />
            ))}
          </>
        )}
      </Stack>
      <ConfirmDialog
        confirmLabel={t('files.confirmRemove')}
        confirmDisabled={!uploads.canUpload}
        destructive
        error={null}
        isPending={uploads.busy}
        onClose={() => setRemoveFile(null)}
        onConfirm={async () => {
          // Close on failure too, so inline server errors are not hidden by the dialog.
          await uploads.changeReferences(
            uploads.uploaded
              .filter((file) => file.id !== removeFile?.id)
              .map((file) => file.id),
          );
          setRemoveFile(null);
        }}
        open={removeFile !== null}
        title={t('files.removeImage', { filename: removeFile?.filename })}
      >
        {t('files.removeImageExplanation')}
      </ConfirmDialog>
    </FormControl>
  );
};

/** Editor for a file attribute; resets its queue when the target changes. */
export const FileAttributeEditor = (props: FileAttributeEditorProps) => (
  <FileAttributeEditorContent
    key={`${props.recordId ?? ''}:${props.attribute.code}:${props.contextId ?? ''}`}
    {...props}
  />
);
