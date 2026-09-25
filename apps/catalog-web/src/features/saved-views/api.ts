import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import { inlineExplorerSearch, type ExplorerSearch } from '../explorer/search';
import { savedViewSchema } from './schemas';

const json = (body: unknown, method: string): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});
const statePayload = (state: ExplorerSearch) => ({
  kind: 'explorer_search',
  state: inlineExplorerSearch(state),
});

export const listSavedViews = (signal?: AbortSignal) =>
  request('/api/saved-views', z.array(savedViewSchema), { signal });
export const getSavedView = (id: string, link: boolean, signal?: AbortSignal) =>
  request(
    `/api/${link ? 'view-state-links' : 'saved-views'}/${encodeURIComponent(id)}`,
    savedViewSchema,
    { signal },
  );
export const createSavedView = (
  name: string,
  description: string,
  visibility: 'private' | 'workspace',
  state: ExplorerSearch,
) =>
  request(
    '/api/saved-views',
    savedViewSchema,
    json({ ...statePayload(state), name, description, visibility }, 'POST'),
  );
export const updateSavedView = (
  id: string,
  name: string,
  description: string,
  visibility: 'private' | 'workspace',
  state: ExplorerSearch,
) =>
  request(
    `/api/saved-views/${encodeURIComponent(id)}`,
    savedViewSchema,
    json({ ...statePayload(state), name, description, visibility }, 'PUT'),
  );
export const deleteSavedView = (id: string) =>
  requestNoContent(`/api/saved-views/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
export const createViewStateLink = (state: ExplorerSearch) =>
  request(
    '/api/view-state-links',
    savedViewSchema,
    json(statePayload(state), 'POST'),
  );
