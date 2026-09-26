import type { Attribute } from './api';
import type { AttributeContext } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';

export const resolvePreviewContext = (
  context: Record<string, Record<string, unknown>>,
  selectedContext: string,
  attributes: readonly Attribute[],
  contexts: readonly AttributeContext[],
): Record<string, unknown> => {
  const byId = new Map(contexts.map((item) => [item.id, item]));
  const selected = contexts.find((item) => item.code === selectedContext);
  const path: string[] = [];
  let current = selected;
  while (current) {
    path.push(current.code);
    current = current.parent_id ? byId.get(current.parent_id) : undefined;
  }
  const selectedExists = path.length > 0;
  if (!selectedExists) path.push(defaultContextCode);
  return Object.fromEntries(
    attributes.flatMap((attribute) => {
      for (const [index, contextCode] of path.entries()) {
        if (
          (!selectedExists || index > 0) &&
          attribute.context_fallback === 'none'
        )
          break;
        const value = context[contextCode]?.[attribute.code];
        if (value !== undefined) return [[attribute.code, value]];
      }
      return [];
    }),
  );
};
