import { useQuery } from '@tanstack/react-query';
import { principalDirectoryOptions } from './queryOptions';

/** Workspace users and teams, for assignment pickers and renderers. */
export const usePrincipalDirectory = (enabled = true) =>
  useQuery({ ...principalDirectoryOptions(), enabled });
