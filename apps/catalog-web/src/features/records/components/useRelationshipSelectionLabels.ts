import { useQueries } from '@tanstack/react-query';
import { getBlueprintByCode, getRecordPreview } from '../api';
import { dropdownOptionLabel } from '../recordDisplay';
import { recordQueryKeys } from '../queryKeys';

const MAX_RESOLVED_RELATIONSHIP_SELECTIONS = 10;

/**
 * Labels selected relationship targets with their blueprint's dropdown view.
 * A relationship may allow several target blueprints; each selected record
 * uses the view of the blueprint it belongs to.
 */
export const useRelationshipSelectionLabels = (
  blueprintCodes: string | string[] | null | undefined,
  ids: string[],
) => {
  const codes = (
    Array.isArray(blueprintCodes) ? blueprintCodes : [blueprintCodes]
  ).filter((code): code is string => Boolean(code));
  const resolvedIds = [...new Set(ids)].slice(
    0,
    MAX_RESOLVED_RELATIONSHIP_SELECTIONS,
  );
  const blueprints = useQueries({
    queries: codes.map((code) => ({
      queryKey: recordQueryKeys.blueprintByCode(code, undefined),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getBlueprintByCode(code, undefined, signal),
      enabled: resolvedIds.length > 0,
    })),
  });
  const previews = useQueries({
    queries: resolvedIds.map((id) => ({
      queryKey: recordQueryKeys.preview(id),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getRecordPreview(id, signal),
      enabled: codes.length > 0,
    })),
  });
  const viewsByBlueprintId = new Map(
    blueprints.flatMap((blueprint) =>
      blueprint.data
        ? [[blueprint.data.blueprint.id, blueprint.data.blueprint.views]]
        : [],
    ),
  );

  return new Map(
    resolvedIds.map((id, index) => {
      const preview = previews[index]?.data;
      const views =
        preview && viewsByBlueprintId.get(preview.record.blueprint_id);
      return [
        id,
        preview && views
          ? (dropdownOptionLabel(preview.context, views) ?? id)
          : id,
      ];
    }),
  );
};
