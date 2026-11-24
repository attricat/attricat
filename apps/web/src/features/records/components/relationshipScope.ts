import { createContext } from 'react';
import type { Attribute } from '../api';
import { relationshipIdsForField } from '../recordForm';

/** A record search filter on a relationship field. */
export type RelationshipTargetFilter = {
  field: string;
  selected_target_ids: string[];
};

/**
 * The current field values of the form a relationship editor is in, by
 * attribute code, so a relationship with a `target_scope` can filter its
 * picker by another field.
 */
export const RelationshipScopeContext = createContext<Readonly<
  Record<string, string>
> | null>(null);

/**
 * The picker filter of a relationship with a `target_scope`: targets whose
 * scoped attribute links to what this record's `equals` field holds. No
 * filter until that field has a value.
 */
export const relationshipScopeFilters = (
  attribute: Attribute,
  formValues: Readonly<Record<string, string>> | null,
): RelationshipTargetFilter[] | undefined => {
  const scope = attribute.target_scope;
  if (!scope || !formValues) return undefined;
  const ids = relationshipIdsForField(formValues[scope.equals] ?? '');
  return ids.length > 0
    ? [{ field: scope.attribute, selected_target_ids: ids }]
    : undefined;
};
