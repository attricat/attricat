import { useCallback, useEffect, useRef, useState } from 'react';
import { randomUuid } from '../../../components/randomUuid';
import { useLatestCallback } from '../../../components/useLatestCallback';

const previewHighlightDuration = 5_000;

const relationshipPickerSearchParameter = 'relationshipPicker';
export const relationshipPickerMessageType =
  'attricat.relationship-picker.select';

export const scrollRelationshipPickerToTop = (element: Element | null) => {
  const content = element?.closest('.MuiDialogContent-root');
  if (content instanceof HTMLElement) content.scrollTo({ top: 0 });
};

type RelationshipPickerMessage = {
  type: typeof relationshipPickerMessageType;
  token: string;
  recordId: string;
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
) => {
  const [previewedIds, setPreviewedIds] = useState<Set<string>>(new Set());
  const [pickerToken] = useState(randomUuid);
  const select = useLatestCallback((id: string) => onSelect?.(id));
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
      if (
        event.origin !== window.location.origin ||
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
  }, [pickerToken, select]);

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
    previewHref,
  };
};
