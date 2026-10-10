import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { uploadFiles } from './api';
import type { PendingFile } from './usePendingFileUploads';

/** Files chosen before their entity exists, keyed by attribute code. */
export type QueuedFiles = Readonly<Record<string, readonly PendingFile[]>>;

export type QueuedFileUploads = {
  pending: QueuedFiles;
  update: (
    attributeCode: string,
    change: (items: PendingFile[]) => PendingFile[],
  ) => void;
};

/** Lets file editors queue files that upload once the entity is created. */
export const QueuedFileUploadsContext = createContext<QueuedFileUploads | null>(
  null,
);

export const useQueuedFileUploadsContext = () =>
  useContext(QueuedFileUploadsContext);

const noQueuedFiles: Record<string, PendingFile[]> = {};

/**
 * Holds files queued for an entity that does not exist yet. Changing `scope`,
 * such as choosing another blueprint, discards the queue.
 */
export const useQueuedFileUploads = (scope: string | undefined) => {
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
  /** Uploads each queued file and returns those that failed, with why. */
  const uploadQueued = async (
    files: QueuedFiles,
    entityId: string,
    contextId: string | null,
  ) => {
    const failed: { filename: string; message?: string }[] = [];
    for (const [attributeCode, items] of Object.entries(files)) {
      for (const item of items) {
        // Zero means "not started", so an upload never reports less than 1.
        const progress = (value: number) =>
          update(attributeCode, (queued) =>
            queued.map((queuedItem) =>
              queuedItem.id === item.id
                ? { ...queuedItem, progress: Math.max(1, value) }
                : queuedItem,
            ),
          );
        progress(1);
        try {
          await uploadFiles({
            entityId,
            attributeCode,
            contextId,
            files: [item.file],
            onProgress: progress,
          });
        } catch (error) {
          failed.push({
            filename: item.file.name,
            message: error instanceof Error ? error.message : undefined,
          });
        }
      }
    }
    return failed;
  };
  return { queue, uploadQueued };
};
