import type { z } from 'zod';
import type { Attribute } from './api';
import { relationshipIdsForField } from './recordForm';
import type { movementSchema } from './schemas';

/** How creating a record changes a balance on the record it finds. */
export type Movement = z.infer<typeof movementSchema>;

/** A record search filter on a relationship field. */
export type RelationshipTargetFilter = {
  field: string;
  selected_target_ids: string[];
};

const noMovements: readonly Movement[] = [];

/** Attributes the server sets on a movement record: its target and result. */
export const movementManagedCodes = (
  movements: readonly Movement[] = noMovements,
): ReadonlySet<string> =>
  new Set(
    movements.flatMap((movement) => [
      movement.target.relationship,
      ...(movement.result ? [movement.result] : []),
    ]),
  );

/**
 * Attributes a movement reads when its record is created. The movement
 * applies once, so the server refuses to change them afterwards.
 */
export const movementInputCodes = (
  movements: readonly Movement[] = noMovements,
): ReadonlySet<string> =>
  new Set(
    movements.flatMap((movement) => [
      movement.quantity,
      ...(movement.factor ? [movement.factor.relationship] : []),
      ...Object.values(movement.target.values),
      ...(movement.reversal ? [movement.reversal.relationship] : []),
    ]),
  );

/** The relationship through which a record reverses another, if any. */
export const reversalRelationship = (
  movements: readonly Movement[] = noMovements,
): string | undefined =>
  movements.find((movement) => movement.reversal)?.reversal?.relationship;

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
