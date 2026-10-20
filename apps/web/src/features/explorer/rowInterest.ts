import { createContext } from 'react';

/**
 * Whether the pointer or keyboard focus is in a result row. Hover-only cell
 * controls, such as the filter shortcut, mount only then, so a page of rows
 * renders without hundreds of hidden buttons. Focus enters a row through its
 * record link or actions before reaching them, so they stay reachable.
 */
export const RowInterestContext = createContext(true);
