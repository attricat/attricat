import { z } from 'zod';
import { apiRequestError, request } from '../../api/request';
import { csrfToken } from '../../api/fetch';
import {
  conversationUploadResultSchema,
  fileMetadataSchema,
  fileUploadResultSchema,
} from './schemas';

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
  if (contextId) data.append('context_id', z.uuid().parse(contextId));
  files.forEach((file) =>
    data.append(files.length === 1 ? 'file' : 'files', file),
  );
  const path = `/api/entities/${encodeURIComponent(z.uuid().parse(entityId))}/file-attributes/${encodeURIComponent(attributeCode)}/uploads`;
  if (typeof XMLHttpRequest === 'undefined') {
    const result = await request(path, fileUploadResultSchema, {
      method: 'POST',
      body: data,
    });
    onProgress?.(100);
    return result;
  }
  return new Promise<z.infer<typeof fileUploadResultSchema>>(
    (resolve, reject) => {
      const request = new XMLHttpRequest();
      request.open('POST', path);
      request.withCredentials = true;
      const csrf = csrfToken();
      if (csrf) request.setRequestHeader('X-Catalog-Csrf', csrf);
      request.upload.onprogress = (event) => {
        if (event.lengthComputable)
          onProgress?.(Math.round((event.loaded / event.total) * 100));
      };
      request.onerror = () =>
        reject(apiRequestError(0, undefined, 'Upload failed'));
      request.onload = () => {
        const body: unknown = (() => {
          try {
            return JSON.parse(request.responseText);
          } catch {
            return undefined;
          }
        })();
        if (request.status < 200 || request.status >= 300) {
          reject(apiRequestError(request.status, body));
          return;
        }
        try {
          onProgress?.(100);
          resolve(fileUploadResultSchema.parse(body));
        } catch (error) {
          reject(error);
        }
      };
      request.send(data);
    },
  );
};

export const uploadConversationFiles = async (
  conversationId: string,
  files: File[],
) => {
  const data = new FormData();
  files.forEach((file) =>
    data.append(files.length === 1 ? 'file' : 'files', file),
  );
  return request(
    `/api/agent/conversations/${encodeURIComponent(z.uuid().parse(conversationId))}/uploads`,
    conversationUploadResultSchema,
    { method: 'POST', body: data },
  );
};
