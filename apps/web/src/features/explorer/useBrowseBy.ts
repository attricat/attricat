import { useQueries } from '@tanstack/react-query';
import { getBlueprintByCode } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import {
  browseByFields,
  getStoredBrowseBy,
  resolveBrowseBy,
  setStoredBrowseBy,
} from './browseBy';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import type { BrowseBy } from './search';

/**
 * The browse rail's candidate fields and current choice for one blueprint.
 * A change is written to the URL, so saved views keep it, and remembered as
 * the person's default for the blueprint.
 */
export const useBrowseBy = ({
  blueprintDefault,
  blueprintId,
  onChange,
  relationshipFields,
  requested,
}: {
  /** The blueprint's `views.explorer.browse_by`. */
  blueprintDefault: string | null | undefined;
  blueprintId: string | undefined;
  onChange: (browseBy: BrowseBy) => void;
  relationshipFields: RelationshipFilterAttribute[];
  requested: BrowseBy | undefined;
}) => {
  // Shares cache entries with relationship filter path discovery.
  const targets = useQueries({
    queries: relationshipFields.map((attribute) => ({
      queryKey: recordQueryKeys.blueprintByCode(
        attribute.target_blueprint_code,
        undefined,
      ),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getBlueprintByCode(attribute.target_blueprint_code, undefined, signal),
    })),
  });
  const stored =
    blueprintId === undefined ? undefined : getStoredBrowseBy(blueprintId);
  const fields = browseByFields(
    relationshipFields,
    targets.map((target) => target.data),
  );
  const pending = targets.some((target) => target.isPending);

  return {
    field: pending
      ? undefined
      : resolveBrowseBy(fields, requested, stored, blueprintDefault),
    fields,
    pending,
    change: (browseBy: BrowseBy) => {
      if (blueprintId !== undefined) setStoredBrowseBy(blueprintId, browseBy);
      onChange(browseBy);
    },
  };
};
