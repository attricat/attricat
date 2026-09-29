export const fileStatuses = {
  uploading: 'uploading',
  queued: 'queued',
  processing: 'processing',
  ready: 'ready',
  failed: 'failed',
  deleted: 'deleted',
} as const;

export const FILE_STATUS_VALUES = [
  fileStatuses.uploading,
  fileStatuses.queued,
  fileStatuses.processing,
  fileStatuses.ready,
  fileStatuses.failed,
  fileStatuses.deleted,
] as const;

export type FileStatus = (typeof FILE_STATUS_VALUES)[number];

/** Statuses after which a file's thumbnail may still become available. */
export const THUMBNAIL_POLLING_STATUSES: ReadonlySet<string> = new Set([
  fileStatuses.uploading,
  fileStatuses.queued,
  fileStatuses.processing,
]);

export const THUMBNAIL_VARIANT_KIND = 'thumbnail';
export const THUMBNAIL_POLL_INTERVAL = 1_000;
export const MAX_THUMBNAIL_RETRIES = 3;
/** Query parameter that busts the browser cache when retrying a thumbnail. */
export const THUMBNAIL_RETRY_QUERY_PARAMETER = 'retry';
/** Upper bound on remembered loaded thumbnail sources. */
export const MAX_LOADED_THUMBNAIL_SOURCES = 256;
export const THUMBNAIL_SPINNER_SIZE = 20;
export const THUMBNAIL_FADE_IN_TRANSITION = 'opacity 200ms ease-in';

/** Thumbnail size for files listed in the attribute editor. */
export const UPLOADED_FILE_THUMBNAIL_SIZE = 48;

export const fileCardinalities = {
  one: 'one',
  many: 'many',
} as const;

export const IMAGE_MIME_PREFIX = 'image/';
export const MIME_GROUP_WILDCARD_SUFFIX = '/*';
export const PENDING_FILE_ID_PREFIX = 'pending-file';

/** Multipart form fields used by upload endpoints. */
export const uploadFormFields = {
  contextId: 'context_id',
  file: 'file',
  files: 'files',
} as const;
