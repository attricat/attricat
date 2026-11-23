import { createContext } from 'react';

/**
 * The current field values of the form a relationship editor is in, by
 * attribute code, so a relationship with a `target_scope` can filter its
 * picker by another field.
 */
export const RelationshipScopeContext = createContext<Readonly<
  Record<string, string>
> | null>(null);
