import { queryOptions } from '@tanstack/react-query';
import {
  listBlueprintFilterReusableAttributes,
  listReusableAttributeGroups,
  listReusableAttributes,
} from './api';
import { reusableAttributeQueryKeys } from './queryKeys';

/** Every reusable attribute revision, with drafts only when asked for. */
export const reusableAttributeListOptions = (includeDrafts = false) =>
  queryOptions({
    queryKey: reusableAttributeQueryKeys.definitions(includeDrafts),
    queryFn: ({ signal }) => listReusableAttributes(includeDrafts, signal),
  });

/** Reusable attributes the Explorer offers as filters on a blueprint. */
export const reusableFilterFieldOptions = (blueprint: string) =>
  queryOptions({
    queryKey: reusableAttributeQueryKeys.filterFields(blueprint),
    queryFn: ({ signal }) =>
      listBlueprintFilterReusableAttributes(blueprint, signal),
  });

export const reusableAttributeGroupListOptions = () =>
  queryOptions({
    queryKey: reusableAttributeQueryKeys.groups(),
    queryFn: ({ signal }) => listReusableAttributeGroups(signal),
  });
