import { useInfiniteQuery } from '@tanstack/react-query';
import { searchRecords } from '../api';
import { displayLabel, dropdownOptionLabel } from '../recordDisplay';
import { recordQueryKeys } from '../queryKeys';
import type { RelationshipTargetFilter } from '../movements';

/**
 * Searches relationship targets and derives their display labels. `filters`
 * narrow the targets, such as a relationship's `target_scope`.
 */
export const useRelationshipTargets = (
  targetBlueprint: string | null | undefined,
  query: string,
  enabled: boolean,
  filters?: readonly RelationshipTargetFilter[],
) => {
  const targets = useInfiniteQuery({
    queryKey: recordQueryKeys.relationshipTargets(
      targetBlueprint,
      query,
      filters,
    ),
    queryFn: ({ pageParam, signal }) =>
      searchRecords({
        blueprint: targetBlueprint!,
        cursor: pageParam,
        query,
        relationshipFilters: filters ? [...filters] : undefined,
        signal,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: Boolean(targetBlueprint && enabled),
  });
  const options = targets.data?.pages.flatMap((page) => page.items) ?? [];
  const targetViews = targets.data?.pages[0]?.blueprint.blueprint.views ?? {};
  const targetLabel = (target: (typeof options)[number]) =>
    dropdownOptionLabel(target.preview, targetViews) ??
    displayLabel(target.display, target.id);
  const labelById = new Map(
    options.map((target) => [target.id, targetLabel(target)]),
  );
  return { labelById, options, targetLabel, targets };
};
