import { useCallback, useEffect, useRef, useState } from 'react';
import { randomUuid } from '../../../components/randomUuid';
import { useLatestCallback } from '../../../components/useLatestCallback';

const previewHighlightDuration = 5_000;

const relationshipPickerSearchParameter = 'relationshipPicker';
export const relationshipPickerMessageType =
  'attricat.relationship-picker.select';
/** Records chosen in an Explorer tab the picker opened. */
export const relationshipPickerManyMessageType =
  'attricat.relationship-picker.select-many';
/** Most records one Explorer choice may return, as the Explorer selects. */
export const MAX_RELATIONSHIP_PICKER_SELECTION = 50;
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export const scrollRelationshipPickerToTop = (element: Element | null) => {
  const content = element?.closest('.MuiDialogContent-root');
  if (content instanceof HTMLElement) content.scrollTo({ top: 0 });
};

type RelationshipPickerMessage = {
  type: typeof relationshipPickerMessageType;
  token: string;
  recordId: string;
};

export type RelationshipPickerManyMessage = {
  type: typeof relationshipPickerManyMessageType;
  token: string;
  recordIds: string[];
  /** Display labels of the records, so the picker can name them at once. */
  labels: Record<string, string>;
};

const isRelationshipPickerManyMessage = (
  value: unknown,
): value is RelationshipPickerManyMessage => {
  if (!value || typeof value !== 'object') return false;
  const message = value as Partial<RelationshipPickerManyMessage>;
  return (
    message.type === relationshipPickerManyMessageType &&
    typeof message.token === 'string' &&
    Array.isArray(message.recordIds) &&
    message.recordIds.length <= MAX_RELATIONSHIP_PICKER_SELECTION &&
    message.recordIds.every(
      (id) => typeof id === 'string' && uuidPattern.test(id),
    ) &&
    typeof message.labels === 'object' &&
    message.labels !== null
  );
};

const isRelationshipPickerMessage = (
  value: unknown,
): value is RelationshipPickerMessage => {
  if (!value || typeof value !== 'object') return false;
  const message = value as Partial<RelationshipPickerMessage>;
  return (
    message.type === relationshipPickerMessageType &&
    typeof message.token === 'string' &&
    typeof message.recordId === 'string'
  );
};

export const useRecentlyPreviewedRecords = (
  onSelect?: (recordId: string) => void,
  onSelectMany?: (recordIds: string[], labels: Record<string, string>) => void,
) => {
  const [previewedIds, setPreviewedIds] = useState<Set<string>>(new Set());
  const [pickerToken] = useState(randomUuid);
  const select = useLatestCallback((id: string) => onSelect?.(id));
  const selectMany = useLatestCallback(
    (ids: string[], labels: Record<string, string>) =>
      onSelectMany?.(ids, labels),
  );
  const leftPicker = useRef(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    const handleBlur = () => {
      leftPicker.current = true;
    };
    const handleFocus = () => {
      if (!leftPicker.current) return;
      leftPicker.current = false;
      clearTimeout(resetTimer.current);
      resetTimer.current = setTimeout(
        () => setPreviewedIds(new Set()),
        previewHighlightDuration,
      );
    };
    const handleMessage = (event: MessageEvent<unknown>) => {
      if (event.origin !== window.location.origin) return;
      if (
        isRelationshipPickerManyMessage(event.data) &&
        event.data.token === pickerToken
      ) {
        const labels = Object.fromEntries(
          event.data.recordIds.flatMap((id) => {
            const label = (event.data as RelationshipPickerManyMessage).labels[
              id
            ];
            return typeof label === 'string' ? [[id, label]] : [];
          }),
        );
        selectMany(event.data.recordIds, labels);
        return;
      }
      if (
        !isRelationshipPickerMessage(event.data) ||
        event.data.token !== pickerToken
      )
        return;
      select(event.data.recordId);
    };
    window.addEventListener('blur', handleBlur);
    window.addEventListener('focus', handleFocus);
    window.addEventListener('message', handleMessage);
    return () => {
      window.removeEventListener('blur', handleBlur);
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('message', handleMessage);
      clearTimeout(resetTimer.current);
    };
  }, [pickerToken, select, selectMany]);

  const markPreviewed = useCallback((id: string) => {
    setPreviewedIds((current) => new Set(current).add(id));
  }, []);
  const previewHref = useCallback(
    (id: string) =>
      `/records/${encodeURIComponent(id)}?${relationshipPickerSearchParameter}=${encodeURIComponent(pickerToken)}`,
    [pickerToken],
  );

  return {
    isPreviewed: (id: string) => previewedIds.has(id),
    markPreviewed,
    pickerToken,
    previewHref,
  };
};

/** Records previewed from a relationship picker, and how to link to them. */
export type RecentlyPreviewedRecords = ReturnType<
  typeof useRecentlyPreviewedRecords
>;
