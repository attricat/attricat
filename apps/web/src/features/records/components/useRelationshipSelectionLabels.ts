import { use } from 'react';
import { KnownRecordLabelsContext } from '../knownRecordLabels';
import { useRecordLabels } from '../useRecordLabels';

/**
 * Labels selected relationship targets with the Default-context display of
 * their own blueprint's dropdown view, so records of different blueprints
 * each get theirs. A record without a label, or that cannot be read, keeps
 * its ID.
 */
export const useRelationshipSelectionLabels = (
  ids: readonly string[],
): ReadonlyMap<string, string> => {
  const knownLabels = use(KnownRecordLabelsContext);
  const selectedIds = [...new Set(ids)];
  // Records the page already labels need no lookup of their own.
  const labels = useRecordLabels(
    selectedIds.filter((id) => !knownLabels.has(id)),
  );
  return new Map(
    selectedIds.map((id) => [id, knownLabels.get(id) ?? labels.get(id) ?? id]),
  );
};
