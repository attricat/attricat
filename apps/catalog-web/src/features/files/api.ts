import { z } from 'zod';
import { request, requestUpload } from '../../api/request';
import {
  conversationUploadResultSchema,
  fileMetadataSchema,
  fileUploadResultSchema,
} from './schemas';
import { uploadFormFields } from './constants';

export const getFileMetadata = (fileId: string) =>
  request(
    `/api/files/${encodeURIComponent(z.uuid().parse(fileId))}`,
    fileMetadataSchema,
  );

export const fileDownloadUrl = (fileId: string, variant?: string) =>
  `/api/files/${encodeURIComponent(z.uuid().parse(fileId))}${variant ? `/variants/${encodeURIComponent(variant)}/download` : '/download'}`;

export const uploadFiles = async ({
  entityId,
  attributeCode,
  contextId,
  files,
  onProgress,
}: {
  entityId: string;
  attributeCode: string;
  contextId?: string | null;
  files: File[];
  onProgress?: (progress: number) => void;
}) => {
  const data = new FormData();
  if (contextId)
    data.append(uploadFormFields.contextId, z.uuid().parse(contextId));
  files.forEach((file) =>
    data.append(
      files.length === 1 ? uploadFormFields.file : uploadFormFields.files,
      file,
    ),
  );
  const path = `/api/entities/${encodeURIComponent(z.uuid().parse(entityId))}/file-attributes/${encodeURIComponent(attributeCode)}/uploads`;
  return requestUpload(path, data, fileUploadResultSchema, onProgress);
};

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
    `/api/agent/conversations/${encodeURIComponent(z.uuid().parse(conversationId))}/uploads`,
    conversationUploadResultSchema,
    { method: 'POST', body: data },
  );
};
