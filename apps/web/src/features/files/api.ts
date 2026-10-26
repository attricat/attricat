import { z } from 'zod';
import { request, requestUpload, jsonRequest } from '../../api/request';
import {
  conversationUploadResultSchema,
  fileMetadataSchema,
  fileUploadResultSchema,
  fileReferencesUpdateResultSchema,
  stagedUploadResultSchema,
  updateFileReferencesSchema,
} from './schemas';
import { uploadFormFields } from './constants';
import { uuidSegment } from '../../api/pathSegments';

export const getFileMetadata = (fileId: string) =>
  request(`/api/files/${uuidSegment(fileId)}`, fileMetadataSchema);

export const fileDownloadUrl = (fileId: string, variant?: string) =>
  `/api/files/${uuidSegment(fileId)}${variant ? `/variants/${encodeURIComponent(variant)}/download` : '/download'}`;

/** The multipart body of an upload: one `file`, or several `files`. */
const uploadFormData = (files: File[], contextId?: string | null) => {
  const data = new FormData();
  if (contextId)
    data.append(uploadFormFields.contextId, z.uuid().parse(contextId));
  files.forEach((file) =>
    data.append(
      files.length === 1 ? uploadFormFields.file : uploadFormFields.files,
      file,
    ),
  );
  return data;
};

export const uploadFiles = async ({
  recordId,
  attributeCode,
  contextId,
  files,
  onProgress,
}: {
  recordId: string;
  attributeCode: string;
  contextId?: string | null;
  files: File[];
  onProgress?: (progress: number) => void;
}) => {
  const data = uploadFormData(files, contextId);
  const path = `/api/records/${uuidSegment(recordId)}/file-attributes/${encodeURIComponent(attributeCode)}/uploads`;
  return requestUpload(path, data, fileUploadResultSchema, onProgress);
};

/**
 * Uploads files for a blueprint's file attribute before the record exists.
 * Creating the record claims them by ID; unclaimed files expire.
 */
export const uploadStagedFiles = async ({
  blueprintId,
  attributeCode,
  contextId,
  files,
  onProgress,
}: {
  blueprintId: string;
  attributeCode: string;
  contextId?: string | null;
  files: File[];
  onProgress?: (progress: number) => void;
}) => {
  const data = uploadFormData(files, contextId);
  const path = `/api/blueprints/${uuidSegment(blueprintId)}/file-attributes/${encodeURIComponent(attributeCode)}/staged-uploads`;
  return requestUpload(path, data, stagedUploadResultSchema, onProgress);
};

export const updateFileReferences = (
  recordId: string,
  attributeCode: string,
  input: z.infer<typeof updateFileReferencesSchema>,
) =>
  request(
    `/api/records/${uuidSegment(recordId)}/file-attributes/${encodeURIComponent(attributeCode)}/references`,
    fileReferencesUpdateResultSchema,
    jsonRequest('PUT', updateFileReferencesSchema.parse(input)),
  );

export const uploadConversationFiles = async (
  conversationId: string,
  files: File[],
) => {
  const data = new FormData();
  files.forEach((file) =>
    data.append(
      files.length === 1 ? uploadFormFields.file : uploadFormFields.files,
      file,
    ),
  );
  return request(
    `/api/agent/conversations/${uuidSegment(conversationId)}/uploads`,
    conversationUploadResultSchema,
    { method: 'POST', body: data },
  );
};
