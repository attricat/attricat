import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import type { NewFileAttributeValue } from '../records/api';
import { uploadStagedFiles } from './api';
import type { PendingFile } from './usePendingFileUploads';

/** Files chosen before their record exists, keyed by attribute code. */
export type QueuedFiles = Readonly<Record<string, readonly PendingFile[]>>;

export type QueuedFileUploads = {
  pending: QueuedFiles;
  update: (
    attributeCode: string,
    change: (items: PendingFile[]) => PendingFile[],
  ) => void;
};

/** Lets file editors queue files that upload when the record is created. */
export const QueuedFileUploadsContext = createContext<QueuedFileUploads | null>(
  null,
);

export const useQueuedFileUploadsContext = () =>
  useContext(QueuedFileUploadsContext);

const noQueuedFiles: Record<string, PendingFile[]> = {};

/**
 * Holds files queued for a record that does not exist yet. Changing `scope`,
 * such as choosing another blueprint, discards the queue.
 */
export const useQueuedFileUploads = (scope: string | undefined) => {
  const { t } = useTranslation();
  const uploadFailed = t('files.uploadFailed');
  const [state, setState] = useState<{
    scope: string | undefined;
    pending: Record<string, PendingFile[]>;
  }>({ scope, pending: {} });
  const pending = state.scope === scope ? state.pending : noQueuedFiles;
  const update = useCallback(
    (attributeCode: string, change: (items: PendingFile[]) => PendingFile[]) =>
      setState((current) => {
        const base = current.scope === scope ? current.pending : {};
        return {
          scope,
          pending: {
            ...base,
            [attributeCode]: change(base[attributeCode] ?? []),
          },
        };
      }),
    [scope],
  );
  const queue = useMemo(() => ({ pending, update }), [pending, update]);
  /**
   * Uploads each queued file that is not staged yet, one at a time, for the
   * blueprint whose record is about to be created. Staged files keep their
   * ID, so a later attempt does not upload them again; failed files keep
   * their error so they can be retried or removed. Returns the file lists to
   * create the record with and the files that failed, with why.
   */
  const stageQueued = async (
    files: QueuedFiles,
    blueprintId: string,
    contextId: string | null,
  ) => {
    const failed: { filename: string; message?: string }[] = [];
    const staged: NewFileAttributeValue[] = [];
    for (const [attributeCode, items] of Object.entries(files)) {
      const fileIds: string[] = [];
      const change = (id: string, next: Partial<PendingFile>) =>
        update(attributeCode, (queued) =>
          queued.map((item) => (item.id === id ? { ...item, ...next } : item)),
        );
      for (const item of items) {
        if (item.stagedFileId) {
          fileIds.push(item.stagedFileId);
          continue;
        }
        // Zero means "not started", so an upload never reports less than 1.
        change(item.id, { error: undefined, progress: 1 });
        try {
          const result = await uploadStagedFiles({
            blueprintId,
            attributeCode,
            contextId,
            files: [item.file],
            onProgress: (progress) =>
              change(item.id, { progress: Math.max(1, progress) }),
          });
          const stagedFileId = result.files[0].id;
          fileIds.push(stagedFileId);
          // Staged files read as ready again and can still be removed.
          change(item.id, { progress: 0, stagedFileId });
        } catch (error) {
          const message = error instanceof Error ? error.message : undefined;
          failed.push({ filename: item.file.name, message });
          change(item.id, { error: message ?? uploadFailed, progress: 0 });
        }
      }
      if (fileIds.length > 0)
        staged.push({
          attribute_code: attributeCode,
          context_id: contextId,
          file_ids: fileIds,
        });
    }
    return { failed, staged };
  };
  /** Uploads every queued file again, such as after its staging expired. */
  const forgetStaged = () =>
    setState((current) => ({
      ...current,
      pending: Object.fromEntries(
        Object.entries(current.pending).map(([code, items]) => [
          code,
          items.map((item) => ({ ...item, stagedFileId: undefined })),
        ]),
      ),
    }));
  return { queue, stageQueued, forgetStaged };
};
