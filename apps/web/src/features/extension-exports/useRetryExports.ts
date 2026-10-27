import { useMutation, useQueryClient } from '@tanstack/react-query';
import { randomUuid } from '../../components/randomUuid';
import { retryExports } from './api';
import { extensionExportQueryKeys } from './queryKeys';

type RetrySelection = Omit<
  Parameters<typeof retryExports>[1],
  'idempotencyKey'
>;

/** Retries exports of an extension, each call as one idempotent request. */
export const useRetryExports = (onRetried?: () => void) => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({
      extensionId,
      selection,
    }: {
      extensionId: string;
      selection: RetrySelection;
    }) =>
      retryExports(extensionId, {
        ...selection,
        idempotencyKey: randomUuid(),
      }),
    onSuccess: () => {
      onRetried?.();
      return client.invalidateQueries({
        queryKey: extensionExportQueryKeys.all,
      });
    },
  });
};
