import { useQueries, useQuery } from '@tanstack/react-query';
import { getBlueprintByCode, getEntityPreview } from '../api';
import { dropdownOptionLabel } from '../entityDisplay';
import { entityQueryKeys } from '../queryKeys';

const MAX_RESOLVED_RELATIONSHIP_SELECTIONS = 10;

export const useRelationshipSelectionLabels = (
  blueprintCode: string | null | undefined,
  ids: string[],
) => {
  const resolvedIds = [...new Set(ids)].slice(
    0,
    MAX_RESOLVED_RELATIONSHIP_SELECTIONS,
  );
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      blueprintCode ?? undefined,
      undefined,
    ),
    queryFn: ({ signal }) =>
      getBlueprintByCode(blueprintCode!, undefined, signal),
    enabled: Boolean(blueprintCode && resolvedIds.length > 0),
  });
  const previews = useQueries({
    queries: resolvedIds.map((id) => ({
      queryKey: entityQueryKeys.preview(id),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getEntityPreview(id, signal),
      enabled: Boolean(blueprintCode),
    })),
  });
  const views = blueprint.data?.blueprint.views ?? {};

  return new Map(
    resolvedIds.map((id, index) => [
      id,
      previews[index]?.data
        ? (dropdownOptionLabel(previews[index].data.context, views) ?? id)
        : id,
    ]),
  );
};
