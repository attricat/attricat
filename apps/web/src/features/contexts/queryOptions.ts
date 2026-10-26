import { queryOptions } from '@tanstack/react-query';
import { listContexts } from './api';
import { contextQueryKeys } from './queryKeys';

/** Every context of the workspace. */
export const contextListOptions = () =>
  queryOptions({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
