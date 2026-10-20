import { useLayoutEffect, useState, type ReactNode } from 'react';
import { createPanelRecordStore, PanelRecordContext } from './panelRecord';

/** Publishes the URL's panel record to the subscribers in `children`. */
export const PanelRecordProvider = ({
  children,
  recordId,
}: {
  children: ReactNode;
  recordId: string | undefined;
}) => {
  const [store] = useState(() => createPanelRecordStore(recordId));
  useLayoutEffect(() => store.set(recordId), [store, recordId]);
  return <PanelRecordContext value={store}>{children}</PanelRecordContext>;
};
