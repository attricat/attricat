import { createContext } from 'react';
import type { ExplorerSort } from './search';

/**
 * The results' sort, read by sortable headers. Column definitions do not
 * close over it, so sorting does not re-render the rows' cells.
 */
export const ExplorerSortContext = createContext<ExplorerSort | undefined>(
  undefined,
);
