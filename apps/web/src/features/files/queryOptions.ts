import { queryOptions } from '@tanstack/react-query';
import { getFileMetadata } from './api';
import {
  THUMBNAIL_POLL_INTERVAL,
  THUMBNAIL_POLLING_STATUSES,
} from './constants';
import { fileQueryKeys } from './queryKeys';

/** A file's metadata, polled while its variants are still being made. */
export const fileMetadataOptions = (fileId: string) =>
  queryOptions({
    queryKey: fileQueryKeys.metadata(fileId),
    queryFn: () => getFileMetadata(fileId),
    retry: false,
    refetchInterval: (query) =>
      !query.state.error &&
      THUMBNAIL_POLLING_STATUSES.has(query.state.data?.status ?? '')
        ? THUMBNAIL_POLL_INTERVAL
        : false,
  });
