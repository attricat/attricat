import { queryOptions } from '@tanstack/react-query';
import { listReusableAttributeGroups, listReusableAttributes } from './api';
import { reusableAttributeQueryKeys } from './queryKeys';

/** Every reusable attribute revision, with drafts only when asked for. */
export const reusableAttributeListOptions = (includeDrafts = false) =>
  queryOptions({
    queryKey: reusableAttributeQueryKeys.definitions(includeDrafts),
    queryFn: ({ signal }) => listReusableAttributes(includeDrafts, signal),
  });

export const reusableAttributeGroupListOptions = () =>
  queryOptions({
    queryKey: reusableAttributeQueryKeys.groups(),
    queryFn: ({ signal }) => listReusableAttributeGroups(signal),
  });
