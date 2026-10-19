import { createContext, useContext, useState } from 'react';

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
