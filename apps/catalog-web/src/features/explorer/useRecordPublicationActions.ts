import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  publishRecord,
  unpublishRecord,
  type RecordItem,
  type RecordPublicationStatus,
} from '../records/api';
import { readinessForContext } from '../records/checkViolations';
import { recordQueryKeys } from '../records/queryKeys';
import {
  recordPublicationOptions,
  recordPublicationReadinessOptions,
} from '../records/queryOptions';
import { invalidateRecordPublications } from '../records/invalidateRecord';

/**
 * Observe only visible rows (plus an open action menu), never all loaded pages.
 * Channel checks are evaluated only for `readinessRecordId` (the open menu).
 */
export const useRecordPublicationActions = (
  items: RecordItem[],
  publicationContextId: string | undefined,
  readinessRecordId?: string,
) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const publicationQueries = useQueries({
    queries: items.map((record) => ({
      ...recordPublicationOptions(record.id),
      enabled: Boolean(publicationContextId),
    })),
  });
  const publicationsByRecordId = new Map(
    items.map((record, index) => [
      record.id,
      publicationQueries[index]?.data?.find(
        (publication) => publication.context_id === publicationContextId,
      ),
    ]),
  );
  const readinessQuery = useQuery({
    ...recordPublicationReadinessOptions(readinessRecordId ?? ''),
    enabled: Boolean(readinessRecordId && publicationContextId),
  });
  const readiness = readinessRecordId
    ? readinessForContext(readinessQuery.data, publicationContextId)
    : undefined;
  const requirePublicationContext = () => {
    if (!publicationContextId)
      throw new Error(t('explorer.publicationContextUnavailable'));
    return publicationContextId;
  };
  const updatePublication = (
    recordId: string,
    publication: RecordPublicationStatus,
  ) =>
    queryClient.setQueryData<RecordPublicationStatus[]>(
      recordQueryKeys.publication(recordId),
      (current) => [
        ...(current ?? []).filter(
          (item) => item.context_id !== publication.context_id,
        ),
        publication,
      ],
    );
  const invalidatePublication = (recordId: string) =>
    invalidateRecordPublications(queryClient, recordId);
  const publish = useMutation({
    mutationFn: (recordId: string) =>
      publishRecord(recordId, requirePublicationContext()),
    onSuccess: (publication, recordId) => {
      updatePublication(recordId, publication);
      void invalidatePublication(recordId);
    },
  });
  const unpublish = useMutation({
    mutationFn: (recordId: string) =>
      unpublishRecord(recordId, requirePublicationContext()),
    onSuccess: (_, recordId) => void invalidatePublication(recordId),
  });

  return {
    error: publish.error ?? unpublish.error,
    publicationsByRecordId,
    publish,
    readiness,
    unpublish,
  };
};
