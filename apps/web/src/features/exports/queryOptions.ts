import { queryOptions } from '@tanstack/react-query';
import { listPublicationChannels } from './api';
import { exportQueryKeys } from './queryKeys';

/** The configured publication channels. */
export const publicationChannelsOptions = () =>
  queryOptions({
    queryKey: exportQueryKeys.channels(),
    queryFn: listPublicationChannels,
  });
