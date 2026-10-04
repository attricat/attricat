import { useQuery } from '@tanstack/react-query';
import { getDirectory } from './api';
import { DIRECTORY_STALE_TIME_MS } from './constants';
import { principalQueryKeys } from './queryKeys';

/** Workspace users and teams, for assignment pickers and renderers. */
export const usePrincipalDirectory = (enabled = true) =>
  useQuery({
    enabled,
    queryKey: principalQueryKeys.directory(),
    queryFn: ({ signal }) => getDirectory(signal),
    staleTime: DIRECTORY_STALE_TIME_MS,
  });
