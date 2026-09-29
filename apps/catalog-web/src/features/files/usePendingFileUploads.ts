import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { uploadFiles } from './api';
import { fileCardinalities } from './constants';
import { acceptsFile, pendingFileId } from './fileAcceptance';
import type { FileMetadata } from './schemas';

export type PendingFile = {
  file: File;
  id: string;
  progress: number;
  error?: string;
};

type Options = {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  entityId?: string;
  files: FileMetadata[];
};

/** Queues files for a file attribute and uploads them one at a time. */
export const usePendingFileUploads = ({
  attribute,
  contextId,
  disabled,
  entityId,
  files,
}: Options) => {
  const { t } = useTranslation();
  const [pending, setPending] = useState<PendingFile[]>([]);
  const [newUploads, setNewUploads] = useState<FileMetadata[]>([]);
  const uploadingIds = useRef(new Set<string>());
  const uploaded = [
    ...files,
    ...newUploads.filter((item) => !files.some((file) => file.id === item.id)),
  ];
  const singleFile =
    attribute.file_policy?.cardinality === fileCardinalities.one;
  const canQueueFile =
    !singleFile || (uploaded.length === 0 && pending.length === 0);
  const canUpload = !disabled && Boolean(entityId);

  const updatePending = (id: string, change: Partial<PendingFile>) =>
    setPending((items) =>
      items.map((value) => (value.id === id ? { ...value, ...change } : value)),
    );

  const add = (candidates: FileList | File[]) => {
    if (!canUpload || !canQueueFile) return;
    const accepted = Array.from(candidates)
      .filter((file) => acceptsFile(file, attribute))
      .slice(0, singleFile ? 1 : undefined);
    setPending((current) => [
      ...current,
      ...accepted.map((file) => ({ file, id: pendingFileId(), progress: 0 })),
    ]);
  };

  const send = async (item: PendingFile) => {
    if (!entityId || disabled || uploadingIds.current.has(item.id)) return;
    uploadingIds.current.add(item.id);
    updatePending(item.id, { error: undefined, progress: 1 });
    try {
      const result = await uploadFiles({
        entityId,
        attributeCode: attribute.code,
        contextId,
        files: [item.file],
        onProgress: (progress) => updatePending(item.id, { progress }),
      });
      setNewUploads((items) => [...items, ...result.files]);
      setPending((items) => items.filter((value) => value.id !== item.id));
    } catch (error) {
      updatePending(item.id, {
        error: error instanceof Error ? error.message : t('files.uploadFailed'),
        progress: 0,
      });
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

  return {
    add,
    canQueueFile,
    canUpload,
    hasQueuedFiles: pending.some((item) => !item.error && item.progress === 0),
    pending,
    retry: (item: PendingFile) => void send(item),
    uploaded,
    uploadPending: () => void uploadPending(),
  };
};
