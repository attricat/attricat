import { createContext, useContext } from 'react';
import type { RelationshipPickerSession } from './search';

/**
 * The relationship picker this Explorer tab chooses records for. The tab
 * then starts in table selection mode and sends its selection back.
 */
export const RelationshipPickerSessionContext =
  createContext<RelationshipPickerSession | null>(null);

export const useRelationshipPickerSession = () =>
  useContext(RelationshipPickerSessionContext);
