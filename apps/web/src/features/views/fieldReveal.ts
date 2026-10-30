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
type FieldRevealRequest = { code: string; focus: boolean; sequence: number };

/**
 * Whether revealing a field also focuses it; leading to a field from
 * elsewhere on the page only scrolls it into view.
 */
export type FieldRevealOptions = { focus?: boolean };

/** The field the user asked to see, so the tab or accordion holding it opens. */
export const FieldRevealContext = createContext<FieldRevealRequest | null>(
  null,
);

/**
 * Shows a field where the fields around it can lead to it, such as from a
 * field's diagnostics; `null` where nothing reveals fields.
 */
export const RevealFieldContext = createContext<
  ((code: string, options?: FieldRevealOptions) => void) | null
>(null);
export const useRevealField = () => useContext(RevealFieldContext);

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
 * `data-field-code`, is focused inside `root`, or only scrolled into view.
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
      // Not implemented by every environment, such as jsdom.
      if (!request.focus) field?.scrollIntoView?.({ block: 'nearest' });
      const shown = request.focus ? focusControlIn(field) : Boolean(field);
      if (shown || --remaining === 0) setRequest(null);
      else frame = requestAnimationFrame(attempt);
    };
    frame = requestAnimationFrame(attempt);
    return () => cancelAnimationFrame(frame);
  }, [request, root]);
  const reveal = useCallback(
    (code: string, { focus = true }: FieldRevealOptions = {}) =>
      setRequest({ code, focus, sequence: ++sequence.current }),
    [],
  );
  return { request, reveal };
};
