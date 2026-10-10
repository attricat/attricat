import type { Attribute } from '../records/api';
import {
  IMAGE_MIME_PREFIX,
  MIME_GROUP_WILDCARD_SUFFIX,
  PENDING_FILE_ID_PREFIX,
  SUPPORTED_IMAGE_MIME_TYPES,
} from './constants';

type FilePolicy = NonNullable<Attribute['file_policy']>;

let pendingFileSequence = 0;

/** Creates a client-side ID for a file queued for upload. */
export const pendingFileId = () => {
  if (
    typeof crypto !== 'undefined' &&
    typeof crypto.randomUUID === 'function'
  ) {
    return crypto.randomUUID();
  }
  pendingFileSequence += 1;
  return `${PENDING_FILE_ID_PREFIX}-${Date.now()}-${pendingFileSequence}`;
};

const normalizedExtension = (extension: string) =>
  extension.replace(/^\./, '').toLowerCase();

/** Value for a file input's `accept` attribute derived from the policy. */
export const acceptedFileTypes = (policy: FilePolicy) =>
  policy.allowed_extensions
    .map((value) => `.${value.replace(/^\./, '')}`)
    .join(',') ||
  (policy.image_only ? [...SUPPORTED_IMAGE_MIME_TYPES].join(',') : undefined);

/** Whether a file satisfies the attribute's file policy. */
export const acceptsFile = (file: File, attribute: Attribute) => {
  const policy = attribute.file_policy;
  if (!policy) return false;
  const extension = file.name.split('.').pop()?.toLowerCase();
  const mimeAllowed = policy.allowed_mime_groups.some((group) =>
    group.endsWith(MIME_GROUP_WILDCARD_SUFFIX)
      ? file.type.startsWith(group.slice(0, -1))
      : file.type === group || file.type.startsWith(`${group}/`),
  );
  return (
    (!policy.max_bytes || file.size <= policy.max_bytes) &&
    (!policy.image_only ||
      (file.type.startsWith(IMAGE_MIME_PREFIX) &&
        SUPPORTED_IMAGE_MIME_TYPES.has(file.type))) &&
    (!policy.allowed_extensions.length ||
      Boolean(
        extension &&
        policy.allowed_extensions.map(normalizedExtension).includes(extension),
      )) &&
    (!policy.allowed_mime_groups.length || mimeAllowed)
  );
};
