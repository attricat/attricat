import { createContext, useContext, useId, useState } from 'react';
import type { Attribute } from './api';
import { attributeCardinalities } from './constants';
import { attributeValueTypes } from './valueTypes';

export type LinkedRecord = { id: string; display?: string };

/**
 * The one record a single relationship links to, or nothing for a list, an
 * empty value or another kind of attribute.
 */
export const singleLinkedRecord = (
  attribute: Attribute,
  value: unknown,
): LinkedRecord | undefined => {
  if (
    attribute.value_type !== attributeValueTypes.relationship ||
    attribute.cardinality !== attributeCardinalities.one ||
    typeof value !== 'object' ||
    value === null ||
    !('items' in value) ||
    !Array.isArray(value.items) ||
    value.items.length !== 1
  )
    return undefined;
  const [item] = value.items as unknown[];
  return typeof item === 'object' &&
    item !== null &&
    'id' in item &&
    typeof item.id === 'string'
    ? (item as LinkedRecord)
    : undefined;
};

/**
 * The context to show linked records in, where a single relationship shows
 * the record it links to within its field; `null` everywhere else.
 */
export const RelatedRecordPreviewContext = createContext<string | null>(null);

export type RelatedRecordPreviewState = {
  contextId: string;
  detailsId: string;
  expanded: boolean;
  recordId: string;
  toggle: () => void;
};

/**
 * Whether a field shows the one record it links to, and whether that record
 * is open. Each newly linked record opens.
 */
export const useRelatedRecordPreview = (
  attribute: Attribute,
  recordId: string | undefined,
): RelatedRecordPreviewState | undefined => {
  const contextId = useContext(RelatedRecordPreviewContext);
  const detailsId = useId();
  const [collapsedId, setCollapsedId] = useState<string>();
  if (
    contextId === null ||
    recordId === undefined ||
    attribute.value_type !== attributeValueTypes.relationship ||
    attribute.cardinality !== attributeCardinalities.one
  )
    return undefined;
  return {
    contextId,
    detailsId,
    expanded: collapsedId !== recordId,
    recordId,
    toggle: () =>
      setCollapsedId((current) =>
        current === recordId ? undefined : recordId,
      ),
  };
};

/**
 * Opens a related record in a new tab, so the record being read or edited
 * stays where it is.
 */
export const relatedRecordLinkProps = {
  rel: 'noopener',
  target: '_blank',
} as const;
