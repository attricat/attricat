import { createContext } from 'react';

/**
 * Whether editors sit in a tight space such as a table cell. Choices then
 * open from a list rather than lying out as radio buttons, and a single
 * relationship leaves out its inline preview.
 */
export const CompactFieldsContext = createContext(false);
