import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
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

// Frames to wait for nested tabs and accordions to open, one after another.
const REVEAL_FRAMES = 30;

/**
 * Calls `onRequest` while rendering, once for each new request, so a tab or
 * collapsed block holding the field opens before the field is focused. A
 * block that mounts while a request is still open, such as one inside a tab
 * the request just selected, handles it too.
 */
export const useFieldRevealRequest = (onRequest: (code: string) => void) => {
  const request = useContext(FieldRevealContext);
  const [handled, setHandled] = useState<FieldRevealRequest | null>(null);
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
  const sequence = useRef(0);
  useEffect(() => {
    if (!request) return;
    // Wait for the tabs and accordions opened for the field to show it, then
    // close the request so blocks that mount later stay as they are.
    let frame = 0;
    let remaining = REVEAL_FRAMES;
    const attempt = () => {
      const field = [
        ...(root.current?.querySelectorAll<HTMLElement>('[data-field-code]') ??
          []),
      ].find((element) => element.dataset.fieldCode === request.code);
      if (focusControlIn(field) || --remaining === 0) setRequest(null);
      else frame = requestAnimationFrame(attempt);
    };
    frame = requestAnimationFrame(attempt);
    return () => cancelAnimationFrame(frame);
  }, [request, root]);
  const reveal = useCallback(
    (code: string) => setRequest({ code, sequence: ++sequence.current }),
    [],
  );
  return { request, reveal };
};
