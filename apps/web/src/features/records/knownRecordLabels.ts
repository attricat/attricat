import { createContext } from 'react';
import type { ResolvedRecordPreview } from './api';

/**
 * Display labels of related records that a page already has, by record ID.
 * Relationship fields use them instead of loading each selected record's
 * preview to label it.
 */
export const KnownRecordLabelsContext = createContext<
  ReadonlyMap<string, string>
>(new Map());

const isLabelledItem = (
  item: unknown,
): item is { id: string; display: string } =>
  typeof item === 'object' &&
  item !== null &&
  'id' in item &&
  typeof item.id === 'string' &&
  'display' in item &&
  typeof item.display === 'string' &&
  item.display !== '';

/**
 * The labels of the targets in a record's relationship values. Only labels
 * resolved in the Default context match the ones relationship fields show.
 */
export const relationshipTargetLabels = (
  preview: ResolvedRecordPreview,
  resolvedInDefaultContext: boolean,
): ReadonlyMap<string, string> => {
  const labels = new Map<string, string>();
  if (!resolvedInDefaultContext) return labels;
  for (const { value } of [
    ...Object.values(preview.values),
    ...Object.values(preview.reusable_values),
  ]) {
    const items =
      typeof value === 'object' && value !== null && 'items' in value
        ? value.items
        : undefined;
    if (!Array.isArray(items)) continue;
    for (const item of items)
      if (isLabelledItem(item)) labels.set(item.id, item.display);
  }
  return labels;
};
