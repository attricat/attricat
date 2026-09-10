import { useCallback, useEffect, useRef, useState } from 'react';

const previewHighlightDuration = 5_000;
let pickerTokenSequence = 0;

const createPickerToken = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function')
    return crypto.randomUUID();
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.getRandomValues === 'function'
  ) {
    const values = crypto.getRandomValues(new Uint32Array(4));
    return [...values].map((value) => value.toString(16)).join('-');
  }
  pickerTokenSequence += 1;
  return `picker-${Date.now()}-${pickerTokenSequence}`;
};

export const relationshipPickerSearchParameter = 'relationshipPicker';
export const relationshipPickerMessageType =
  'attricat.relationship-picker.select';

type RelationshipPickerMessage = {
  type: typeof relationshipPickerMessageType;
  token: string;
  entityId: string;
};

const isRelationshipPickerMessage = (
  value: unknown,
): value is RelationshipPickerMessage => {
  if (!value || typeof value !== 'object') return false;
  const message = value as Partial<RelationshipPickerMessage>;
  return (
    message.type === relationshipPickerMessageType &&
    typeof message.token === 'string' &&
    typeof message.entityId === 'string'
  );
};

export const useRecentlyPreviewedEntities = (
  onSelect?: (entityId: string) => void,
) => {
  const [previewedIds, setPreviewedIds] = useState<Set<string>>(new Set());
  const [pickerToken] = useState(createPickerToken);
  const selectCallback = useRef(onSelect);
  const leftPicker = useRef(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    selectCallback.current = onSelect;
  }, [onSelect]);

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
      selectCallback.current?.(event.data.entityId);
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
  }, [pickerToken]);

  const markPreviewed = useCallback((id: string) => {
    setPreviewedIds((current) => new Set(current).add(id));
  }, []);
  const previewHref = useCallback(
    (id: string) =>
      `/entities/${encodeURIComponent(id)}?${relationshipPickerSearchParameter}=${encodeURIComponent(pickerToken)}`,
    [pickerToken],
  );
  const openPreview = useCallback(
    (id: string) => {
      markPreviewed(id);
      window.open(previewHref(id), '_blank');
    },
    [markPreviewed, previewHref],
  );

  return {
    isPreviewed: (id: string) => previewedIds.has(id),
    markPreviewed,
    openPreview,
    previewHref,
  };
};
