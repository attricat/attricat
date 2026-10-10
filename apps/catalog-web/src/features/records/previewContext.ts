import type { Attribute } from './api';
import type { AttributeContext } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import { attributeContextFallbacks } from './constants';

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

export const resolvePreviewContext = (
  context: Record<string, Record<string, unknown>>,
  selectedContext: string,
  attributes: readonly Attribute[],
  contexts: readonly AttributeContext[],
): Record<string, unknown> => {
  const path = contextAncestorCodes(contexts, selectedContext);
  const selectedExists = path.length > 0;
  if (!selectedExists) path.push(defaultContextCode);
  return Object.fromEntries(
    attributes.flatMap((attribute) => {
      for (const [index, contextCode] of path.entries()) {
        if (
          (!selectedExists || index > 0) &&
          attribute.context_fallback === attributeContextFallbacks.none
        )
          break;
        const value = context[contextCode]?.[attribute.code];
        if (value !== undefined) return [[attribute.code, value]];
      }
      return [];
    }),
  );
};
