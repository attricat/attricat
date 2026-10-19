import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type RefObject,
} from 'react';
import { focusControlIn } from '../../components/initialFocus';

/** A request to show a field; each request has a new `sequence`. */
export type FieldRevealRequest = { code: string; sequence: number };

/** The field the user asked to see, so the tab or accordion holding it opens. */
export const FieldRevealContext = createContext<FieldRevealRequest | null>(
  null,
);

/**
 * Calls `onRequest` while rendering, once for each new request, so a tab or
 * collapsed block holding the field opens before the field is focused.
 */
export const useFieldRevealRequest = (onRequest: (code: string) => void) => {
  const request = useContext(FieldRevealContext);
  const [handled, setHandled] = useState(request);
  if (request !== handled) {
    setHandled(request);
    if (request) onRequest(request.code);
  }
};

/**
 * Shows a field on request: provide `request` through `FieldRevealContext` so
 * the tab or accordion holding it opens, and the field, marked with
 * `data-field-code`, is focused inside `root`.
 */
export const useFieldReveal = (root: RefObject<HTMLElement | null>) => {
  const [request, setRequest] = useState<FieldRevealRequest | null>(null);
  useEffect(() => {
    if (!request) return;
    // Wait a frame so a tab or accordion opened for the field shows it.
    const frame = requestAnimationFrame(() =>
      focusControlIn(
        [
          ...(root.current?.querySelectorAll<HTMLElement>(
            '[data-field-code]',
          ) ?? []),
        ].find((element) => element.dataset.fieldCode === request.code),
      ),
    );
    return () => cancelAnimationFrame(frame);
  }, [request, root]);
  const reveal = useCallback(
    (code: string) =>
      setRequest((current) => ({
        code,
        sequence: (current?.sequence ?? 0) + 1,
      })),
    [],
  );
  return { request, reveal };
};
