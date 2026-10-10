import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../records/api';
import { invalidateRecord } from '../records/invalidateRecord';
import { uploadFiles, updateFileReferences } from './api';
import { fileCardinalities } from './constants';
import { acceptsFile, pendingFileId } from './fileAcceptance';
import { useQueuedFileUploadsContext } from './queuedFileUploads';
import type { FileMetadata } from './schemas';

export type PendingFile = {
  file: File;
  id: string;
  progress: number;
  error?: string;
  /** Set once a file queued before its record exists has been staged. */
  stagedFileId?: string;
};

type Options = {
  attribute: Attribute;
  contextId: string | null;
  disabled: boolean;
  recordId?: string;
  files: FileMetadata[];
  /** Called with the record version produced by this editor's own change. */
  onRecordUpdated?: (updatedAt: string) => void;
};

/** Files save separately from scalar form values. Never persist file inputs in drafts. */
export const usePendingFileUploads = (options: Options) => {
  const { attribute, contextId, disabled, recordId, files } = options;
  const { t } = useTranslation();
  const client = useQueryClient();
  // Before the record exists, files wait in the creating page's queue.
  const queue = useQueuedFileUploadsContext();
  const deferred = !recordId && queue !== null;
  const [localPending, setLocalPending] = useState<PendingFile[]>([]);
  const pending: readonly PendingFile[] = deferred
    ? (queue.pending[attribute.code] ?? [])
    : localPending;
  const setPending = (change: (items: PendingFile[]) => PendingFile[]) =>
    deferred ? queue.update(attribute.code, change) : setLocalPending(change);
  const [saved, setSaved] = useState<{
    source: FileMetadata[];
    value: FileMetadata[];
  } | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const current = useRef(options);
  const active = useRef(true);
  useEffect(() => {
    current.current = options;
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  // A refetch is authoritative, including files removed by another editor.
  const uploaded = saved?.source === files ? saved.value : files;
  const singleFile =
    attribute.file_policy?.cardinality === fileCardinalities.one;
  const canQueueFile =
    !singleFile || (uploaded.length === 0 && pending.length === 0);
  const canUpload = !disabled && Boolean(recordId) && !busy;
  const canQueue = canUpload || (!disabled && deferred);
  const permitted = () =>
    active.current &&
    !current.current.disabled &&
    Boolean(current.current.recordId);
  const queuePermitted = () =>
    active.current && !current.current.disabled && (deferred || permitted());
  const refresh = async () => {
    if (!recordId) return;
    await invalidateRecord(client, recordId);
  };
  const add = (candidates: FileList | File[]) => {
    if (!canQueue || !canQueueFile || lock.current || !queuePermitted()) return;
    const rejected: string[] = [];
    const accepted = Array.from(candidates).filter((file) => {
      if (acceptsFile(file, attribute)) return true;
      rejected.push(t('files.fileRejected', { filename: file.name }));
      return false;
    });
    if (singleFile && accepted.length > 1)
      rejected.push(t('files.singleFileOnly'));
    setErrors(rejected);
    setPending((items) => [
      ...items,
      ...accepted
        .slice(0, singleFile ? 1 : undefined)
        .map((file) => ({ file, id: pendingFileId(), progress: 0 })),
    ]);
  };
  const updatePending = (id: string, change: Partial<PendingFile>) =>
    setPending((items) =>
      items.map((item) => (item.id === id ? { ...item, ...change } : item)),
    );
  const upload = async (items: PendingFile[]) => {
    if (lock.current || !permitted()) return;
    lock.current = true;
    setBusy(true);
    let next = uploaded;
    try {
      for (const item of items) {
        if (!permitted()) break;
        updatePending(item.id, { error: undefined, progress: 1 });
        try {
          const result = await uploadFiles({
            recordId: recordId!,
            attributeCode: attribute.code,
            contextId,
            files: [item.file],
            // Zero means "queued"; an early 0% event must not show the file
            // as ready and removable mid-upload.
            onProgress: (progress) =>
              updatePending(item.id, { progress: Math.max(1, progress) }),
          });
          next = [...next, ...result.files];
          current.current.onRecordUpdated?.(result.record_updated_at);
          if (active.current) {
            setSaved({ source: current.current.files, value: next });
            setPending((queue) =>
              queue.filter((value) => value.id !== item.id),
            );
          }
        } catch (error) {
          if (active.current)
            updatePending(item.id, {
              error:
                error instanceof Error
                  ? error.message
                  : t('files.uploadFailed'),
              progress: 0,
            });
        }
      }
      await refresh();
    } finally {
      lock.current = false;
      if (active.current) setBusy(false);
    }
  };
  const changeReferences = async (fileIds: string[]) => {
    if (lock.current || !permitted()) return false;
    lock.current = true;
    setBusy(true);
    setErrors([]);
    try {
      const result = await updateFileReferences(recordId!, attribute.code, {
        context_id: contextId,
        expected_file_ids: uploaded.map((file) => file.id),
        file_ids: fileIds,
      });
      current.current.onRecordUpdated?.(result.record_updated_at);
      if (active.current)
        setSaved({
          source: current.current.files,
          value: fileIds.map((id) => uploaded.find((file) => file.id === id)!),
        });
      await refresh();
      return true;
    } catch (error) {
      if (active.current)
        setErrors([
          error instanceof Error ? error.message : t('files.updateFailed'),
        ]);
      await refresh();
      return false;
    } finally {
      lock.current = false;
      if (active.current) setBusy(false);
    }
  };
  return {
    add,
    busy,
    errors,
    canQueue,
    canQueueFile,
    canUpload,
    deferred,
    changeReferences,
    hasQueuedFiles: pending.some((item) => !item.error && item.progress === 0),
    pending,
    uploaded,
    removePending: (id: string) => {
      if (queuePermitted() && !lock.current)
        setPending((items) => items.filter((item) => item.id !== id));
    },
    // A queued file is staged again when the record is created.
    retry: (item: PendingFile) =>
      deferred
        ? updatePending(item.id, { error: undefined, progress: 0 })
        : void upload([item]),
    uploadPending: () =>
      void upload(pending.filter((item) => !item.error && item.progress === 0)),
  };
};
