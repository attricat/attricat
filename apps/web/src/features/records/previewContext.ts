import type { AttributeContext } from '../contexts/api';

/** Codes of the context `code` and its ancestors, nearest first. */
export const contextAncestorCodes = (
  contexts: readonly AttributeContext[],
  code: string,
): string[] => {
  const byId = new Map(contexts.map((item) => [item.id, item]));
  const path: string[] = [];
  let current = contexts.find((item) => item.code === code);
  while (current && !path.includes(current.code)) {
    path.push(current.code);
    current = current.parent_id ? byId.get(current.parent_id) : undefined;
  }
  return path;
};
