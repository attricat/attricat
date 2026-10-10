export const SAVED_VIEW_VISIBILITY_PRIVATE = 'private';
export const SAVED_VIEW_VISIBILITY_WORKSPACE = 'workspace';
export type SavedViewVisibility =
  typeof SAVED_VIEW_VISIBILITY_PRIVATE | typeof SAVED_VIEW_VISIBILITY_WORKSPACE;

export const MAXIMUM_INLINE_LINK_LENGTH = 1800;
export const SAVED_VIEW_NAME_MAX_LENGTH = 120;
export const SAVED_VIEW_DESCRIPTION_MAX_LENGTH = 500;
export const SAVED_VIEW_SEARCH_MAX_LENGTH = 120;
export const SAVED_VIEW_SEARCH_DEBOUNCE_MS = 250;
export const SAVE_SEARCH_FORM_ID = 'save-search-form';

export const SOURCE_VIEW_PARAM = 'sourceView';
export const SAVED_VIEW_PARAM = 'savedView';
export const VIEW_STATE_PARAM = 'viewState';

export const SAVED_VIEWS_PATH = '/api/saved-views';
export const VIEW_STATE_LINKS_PATH = '/api/view-state-links';
export const EXPLORER_SEARCH_KIND = 'explorer_search';
