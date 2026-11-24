import { createContext } from 'react';

/**
 * Whether views may change the records they show, such as adding an
 * order's lines in its table. A preview of an existing record must not.
 */
export const RecordWritesContext = createContext(true);
