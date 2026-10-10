import { queryOptions } from '@tanstack/react-query';
import { getDirectory, listTeams } from './api';
import { DIRECTORY_STALE_TIME_MS } from './constants';
import { principalQueryKeys } from './queryKeys';

/** Workspace users and teams, for assignment pickers and renderers. */
export const principalDirectoryOptions = () =>
  queryOptions({
    queryKey: principalQueryKeys.directory(),
    queryFn: ({ signal }) => getDirectory(signal),
    staleTime: DIRECTORY_STALE_TIME_MS,
  });

/** Teams with their members, for workspace management. */
export const teamListOptions = () =>
  queryOptions({
    queryKey: principalQueryKeys.teams(),
    queryFn: listTeams,
  });
