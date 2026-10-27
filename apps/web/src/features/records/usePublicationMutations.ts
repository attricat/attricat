import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  publishRecord,
  unpublishRecord,
  type RecordPublicationStatus,
} from './api';
import { invalidateRecordPublications } from './invalidateRecord';
import { recordQueryKeys } from './queryKeys';

type PublicationTarget = { recordId: string; contextId: string };

/**
 * Publishes or unpublishes one record in one channel. A publish shows its
 * result at once; both settle once the record's publications are refetched.
 */
export const usePublicationMutations = () => {
  const client = useQueryClient();
  const publish = useMutation({
    mutationFn: ({ recordId, contextId }: PublicationTarget) =>
      publishRecord(recordId, contextId),
    onSuccess: (publication, { recordId }) => {
      client.setQueryData<RecordPublicationStatus[]>(
        recordQueryKeys.publication(recordId),
        (current) => [
          ...(current ?? []).filter(
            (item) => item.context_id !== publication.context_id,
          ),
          publication,
        ],
      );
      return invalidateRecordPublications(client, recordId);
    },
  });
  const unpublish = useMutation({
    mutationFn: ({ recordId, contextId }: PublicationTarget) =>
      unpublishRecord(recordId, contextId),
    onSuccess: (_, { recordId }) =>
      invalidateRecordPublications(client, recordId),
  });
  return { publish, unpublish };
};
