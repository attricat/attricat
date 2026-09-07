import CloudUploadOutlinedIcon from '@mui/icons-material/CloudUploadOutlined';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutlineOutlined';
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

export const FileAttributeEditor = ({
  attribute,
  contextId,
  disabled,
  entityId,
  files,
}: {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  entityId?: string;
  files: FileMetadata[];
}) => {
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const [pending, setPending] = useState<PendingFile[]>([]);
  const [uploaded, setUploaded] = useState<FileMetadata[]>(files);
  const policy = attribute.file_policy;
  if (!policy) return null;

  const add = (files: FileList | File[]) => {
    const accepted = Array.from(files).filter((file) =>
      acceptsFile(file, attribute),
    );
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
    if (!entityId) return;
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
      setUploaded((items) => [...items, ...result.files]);
      setPending((items) => items.filter((value) => value.id !== item.id));
    } catch (error) {
      setPending((items) =>
        items.map((value) =>
          value.id === item.id
            ? {
                ...value,
                error: error instanceof Error ? error.message : 'Upload failed',
                progress: 0,
              }
            : value,
        ),
      );
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
  const reorder = (index: number, direction: -1 | 1) =>
    setUploaded((items) => {
      const next = [...items];
      const target = index + direction;
      if (target < 0 || target >= next.length) return items;
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });

  return (
    <Stack spacing={1}>
      <Typography>{attribute.code}</Typography>
      {!entityId && (
        <Alert severity="info">Save the entity before uploading files.</Alert>
      )}
      <Box
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          if (!disabled) add(event.dataTransfer.files);
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
          hidden
          multiple={policy.cardinality === 'many'}
          onChange={(event) => {
            if (!disabled && event.target.files) add(event.target.files);
            event.target.value = '';
          }}
          ref={input}
          type="file"
        />
        <Button
          disabled={disabled || !entityId}
          onClick={() => input.current?.click()}
          startIcon={<CloudUploadOutlinedIcon />}
        >
          Choose or drop files
        </Button>
        {pending.length > 0 && (
          <Button
            color="primary"
            disabled={disabled || !entityId}
            onClick={startUploads}
            variant="contained"
          >
            Upload {pending.length} file{pending.length === 1 ? '' : 's'}
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
                  aria-label={`Retry ${item.file.name}`}
                  disabled={disabled}
                  onClick={() => void send(item)}
                >
                  <ReplayOutlinedIcon />
                </IconButton>
              </>
            ) : (
              <Chip
                label={item.progress ? `${item.progress}%` : 'Ready'}
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
      {uploaded.map((file, index) => (
        <Stack alignItems="center" direction="row" key={file.id} spacing={1}>
          {policy.image_only && <FileThumbnail file={file} size={48} />}
          <Typography sx={{ flexGrow: 1 }}>{file.filename}</Typography>
          <Chip label={file.status} size="small" />
          <IconButton
            aria-label={`Download ${file.filename}`}
            component="a"
            href={fileDownloadUrl(file.id)}
          >
            <DownloadOutlinedIcon />
          </IconButton>
          {policy.ordered && (
            <>
              <Button
                disabled={disabled || index === 0}
                onClick={() => reorder(index, -1)}
              >
                Up
              </Button>
              <Button
                disabled={disabled || index === uploaded.length - 1}
                onClick={() => reorder(index, 1)}
              >
                Down
              </Button>
            </>
          )}
          <IconButton
            aria-label={`Remove ${file.filename}`}
            disabled={disabled}
            onClick={() =>
              setUploaded((items) =>
                items.filter((value) => value.id !== file.id),
              )
            }
          >
            <DeleteOutlineIcon />
          </IconButton>
        </Stack>
      ))}
    </Stack>
  );
};
