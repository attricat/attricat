import CloudUploadOutlinedIcon from '@mui/icons-material/CloudUploadOutlined';
import DownloadOutlinedIcon from '@mui/icons-material/DownloadOutlined';
import ReplayOutlinedIcon from '@mui/icons-material/ReplayOutlined';
import {
  Alert,
  Box,
  Button,
  Chip,
  IconButton,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { fileDownloadUrl, uploadFiles } from './api';
import { FileThumbnail } from './FileThumbnail';
import type { FileMetadata } from './schemas';

type PendingFile = {
  file: File;
  id: string;
  progress: number;
  error?: string;
};

let pendingFileSequence = 0;

const pendingFileId = () => {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return crypto.randomUUID();
  }
  pendingFileSequence += 1;
  return `pending-file-${Date.now()}-${pendingFileSequence}`;
};

const acceptsFile = (file: File, attribute: Attribute) => {
  const policy = attribute.file_policy;
  if (!policy) return false;
  const extension = file.name.split('.').pop()?.toLowerCase();
  const mimeAllowed = policy.allowed_mime_groups.some((group) =>
    group.endsWith('/*')
      ? file.type.startsWith(group.slice(0, -1))
      : file.type === group || file.type.startsWith(`${group}/`),
  );
  return (
    (!policy.max_bytes || file.size <= policy.max_bytes) &&
    (!policy.image_only || file.type.startsWith('image/')) &&
    (!policy.allowed_extensions.length ||
      Boolean(
        extension &&
        policy.allowed_extensions
          .map((value) => value.replace(/^\./, '').toLowerCase())
          .includes(extension),
      )) &&
    (!policy.allowed_mime_groups.length || mimeAllowed)
  );
};

type FileAttributeEditorProps = {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  entityId?: string;
  files: FileMetadata[];
};

const FileAttributeEditorContent = ({
  attribute,
  contextId,
  disabled,
  entityId,
  files,
}: FileAttributeEditorProps) => {
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState<PendingFile[]>([]);
  const [newUploads, setNewUploads] = useState<FileMetadata[]>([]);
  const uploadingIds = useRef(new Set<string>());
  const uploaded = [
    ...files,
    ...newUploads.filter((item) => !files.some((file) => file.id === item.id)),
  ];
  const policy = attribute.file_policy;
  if (!policy) return null;
  const canQueueFile =
    policy.cardinality !== 'one' ||
    (uploaded.length === 0 && pending.length === 0);

  const add = (files: FileList | File[]) => {
    if (!canQueueFile) return;
    const accepted = Array.from(files)
      .filter((file) => acceptsFile(file, attribute))
      .slice(0, policy.cardinality === 'one' ? 1 : undefined);
    setPending((current) => [
      ...current,
      ...accepted.map((file) => ({
        file,
        id: pendingFileId(),
        progress: 0,
      })),
    ]);
  };
  const send = async (item: PendingFile) => {
    if (!entityId || disabled || uploadingIds.current.has(item.id)) return;
    uploadingIds.current.add(item.id);
    setPending((items) =>
      items.map((value) =>
        value.id === item.id
          ? { ...value, error: undefined, progress: 1 }
          : value,
      ),
    );
    try {
      const result = await uploadFiles({
        entityId,
        attributeCode: attribute.code,
        contextId,
        files: [item.file],
        onProgress: (progress) =>
          setPending((items) =>
            items.map((value) =>
              value.id === item.id ? { ...value, progress } : value,
            ),
          ),
      });
      setNewUploads((items) => [...items, ...result.files]);
      setPending((items) => items.filter((value) => value.id !== item.id));
    } catch (error) {
      setPending((items) =>
        items.map((value) =>
          value.id === item.id
            ? {
                ...value,
                error:
                  error instanceof Error
                    ? error.message
                    : t('files.uploadFailed'),
                progress: 0,
              }
            : value,
        ),
      );
    } finally {
      uploadingIds.current.delete(item.id);
    }
  };
  const uploadPending = async () => {
    for (const item of pending.filter(
      (item) => !item.error && item.progress === 0,
    )) {
      await send(item);
    }
  };
  const startUploads = () => {
    void uploadPending();
  };
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
          if (!disabled && entityId && canQueueFile)
            add(event.dataTransfer.files);
        }}
        sx={{
          border: '1px dashed',
          borderColor: 'divider',
          borderRadius: 1,
          p: 2,
        }}
      >
        <input
          accept={
            policy.allowed_extensions
              .map((value) => `.${value.replace(/^\./, '')}`)
              .join(',') || undefined
          }
          disabled={disabled || !entityId || !canQueueFile}
          hidden
          multiple={policy.cardinality === 'many'}
          onChange={(event) => {
            if (!disabled && entityId && canQueueFile && event.target.files)
              add(event.target.files);
            event.target.value = '';
          }}
          ref={input}
          type="file"
        />
        <Button
          disabled={disabled || !entityId || !canQueueFile}
          onClick={() => input.current?.click()}
          startIcon={<CloudUploadOutlinedIcon />}
        >
          {t('files.chooseOrDropFiles')}
        </Button>
        {pending.length > 0 && (
          <Button
            color="primary"
            disabled={
              disabled ||
              !entityId ||
              pending.every((item) => item.error || item.progress > 0)
            }
            onClick={startUploads}
            variant="contained"
          >
            {t('files.uploadCount', { count: pending.length })}
          </Button>
        )}
      </Box>
      {pending.map((item) => (
        <Box key={item.id}>
          <Stack direction="row" spacing={1}>
            <Typography>{item.file.name}</Typography>
            {item.error ? (
              <>
                <Chip color="error" label={t('files.failed')} size="small" />
                <IconButton
                  aria-label={t('files.retryFile', {
                    filename: item.file.name,
                  })}
                  disabled={disabled}
                  onClick={() => void send(item)}
                >
                  <ReplayOutlinedIcon />
                </IconButton>
              </>
            ) : (
              <Chip
                label={item.progress ? `${item.progress}%` : t('files.ready')}
                size="small"
              />
            )}
          </Stack>
          {item.progress > 0 && (
            <LinearProgress value={item.progress} variant="determinate" />
          )}
          {item.error && (
            <Typography color="error" variant="body2">
              {item.error}
            </Typography>
          )}
        </Box>
      ))}
      {uploaded.map((file) => (
        <Stack
          direction="row"
          key={file.id}
          spacing={1}
          sx={{ alignItems: 'center' }}
        >
          {policy.image_only && <FileThumbnail file={file} size={48} />}
          <Typography sx={{ flexGrow: 1 }}>{file.filename}</Typography>
          <Chip label={file.status} size="small" />
          <IconButton
            aria-label={t('files.downloadFile', { filename: file.filename })}
            component="a"
            href={fileDownloadUrl(file.id)}
          >
            <DownloadOutlinedIcon />
          </IconButton>
        </Stack>
      ))}
    </Stack>
  );
};

export const FileAttributeEditor = (props: FileAttributeEditorProps) => (
  <FileAttributeEditorContent
    key={`${props.entityId ?? ''}:${props.attribute.code}:${props.contextId ?? ''}`}
    {...props}
  />
);
