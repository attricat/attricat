import { queryOptions } from '@tanstack/react-query';
import { listAssignableRoles, listRoles } from './api';
import { workspaceQueryKeys } from './queryKeys';

/** The workspace's roles. */
export const roleListOptions = () =>
  queryOptions({
    queryKey: workspaceQueryKeys.roles(),
    queryFn: listRoles,
  });

/** The roles the current user may grant. */
export const assignableRolesOptions = () =>
  queryOptions({
    queryKey: workspaceQueryKeys.assignableRoles(),
    queryFn: listAssignableRoles,
  });
