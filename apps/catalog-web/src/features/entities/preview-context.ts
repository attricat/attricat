import type { Attribute } from './api';

export const resolvePreviewContext = (
  context: Record<string, Record<string, unknown>>,
  selectedContext: string,
  attributes: readonly Attribute[],
): Record<string, unknown> => {
  const local = context[selectedContext] ?? {};
  const fallback = context.default ?? {};
  return Object.fromEntries(
    attributes.flatMap((attribute) => {
      if (attribute.code in local) return [[attribute.code, local[attribute.code]]];
      if (selectedContext !== 'default' && attribute.context_fallback !== 'none') {
        const value = fallback[attribute.code];
        if (value !== undefined) return [[attribute.code, value]];
      }
      return [];
    }),
  );
};
