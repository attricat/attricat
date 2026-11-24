import type { z } from 'zod';
import type { movementSchema } from './schemas';

/**
 * How a record changes a balance on the record it finds, when it is created
 * or when a status change triggers it.
 */
export type Movement = z.infer<typeof movementSchema>;

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
 * Attributes a movement reads when it applies: on create, or at a status
 * change such as an order's approval. It applies once, so the server
 * refuses to change them afterwards. With `applied`, only the inputs of
 * those movements.
 */
export const movementInputCodes = (
  movements: readonly Movement[] = noMovements,
  applied?: readonly string[],
): ReadonlySet<string> =>
  new Set(
    movements
      .filter((movement) => !applied || applied.includes(movement.code))
      .flatMap((movement) => [
        movement.quantity,
        ...(movement.factor ? [movement.factor.relationship] : []),
        ...Object.values(movement.target.values),
        ...(movement.reversal ? [movement.reversal.relationship] : []),
        ...(movement.apply_on ?? []).flatMap((trigger) =>
          trigger.relationship ? [trigger.relationship] : [],
        ),
      ]),
  );

/** The relationship through which a record reverses another, if any. */
export const reversalRelationship = (
  movements: readonly Movement[] = noMovements,
): string | undefined =>
  movements.find((movement) => movement.reversal)?.reversal?.relationship;
