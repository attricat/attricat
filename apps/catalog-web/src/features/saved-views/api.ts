import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import { inlineExplorerSearch, type ExplorerSearch } from '../explorer/search';
import {
  EXPLORER_SEARCH_KIND,
  SAVED_VIEWS_PATH,
  VIEW_STATE_LINKS_PATH,
  type SavedViewVisibility,
} from './constants';
import { savedViewSchema } from './schemas';

const json = (body: unknown, method: string): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});
const statePayload = (state: ExplorerSearch) => ({
  kind: EXPLORER_SEARCH_KIND,
  state: inlineExplorerSearch(state),
});

export const listSavedViews = (signal?: AbortSignal) =>
  request(SAVED_VIEWS_PATH, z.array(savedViewSchema), { signal });
export const getSavedView = (id: string, link: boolean, signal?: AbortSignal) =>
  request(
    `${link ? VIEW_STATE_LINKS_PATH : SAVED_VIEWS_PATH}/${encodeURIComponent(id)}`,
    savedViewSchema,
    { signal },
  );
export const createSavedView = (
  name: string,
  description: string,
  visibility: SavedViewVisibility,
  state: ExplorerSearch,
) =>
  request(
    SAVED_VIEWS_PATH,
    savedViewSchema,
    json({ ...statePayload(state), name, description, visibility }, 'POST'),
  );
export const updateSavedView = (
  id: string,
  name: string,
  description: string,
  visibility: SavedViewVisibility,
  state: ExplorerSearch,
) =>
  request(
    `${SAVED_VIEWS_PATH}/${encodeURIComponent(id)}`,
    savedViewSchema,
    json({ ...statePayload(state), name, description, visibility }, 'PUT'),
  );
export const deleteSavedView = (id: string) =>
  requestNoContent(`${SAVED_VIEWS_PATH}/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
export const createViewStateLink = (state: ExplorerSearch) =>
  request(
    VIEW_STATE_LINKS_PATH,
    savedViewSchema,
    json(statePayload(state), 'POST'),
  );
