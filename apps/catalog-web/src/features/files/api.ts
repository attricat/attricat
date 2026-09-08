import { z } from 'zod';
import { apiFetch, csrfToken } from '../auth/request';
import { ApiRequestError } from '../entities/api';
import {
  conversationUploadResultSchema,
  fileMetadataSchema,
  fileUploadResultSchema,
} from './schemas';

const apiErrorSchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
});

const responseError = async (response: Response) => {
  const result = apiErrorSchema.safeParse(
    await response.json().catch(() => null),
  );
  return new ApiRequestError(
    response.status,
    result.success
      ? result.data.error.message
      : `Request failed (${response.status})`,
    result.success ? result.data.error.code : undefined,
  );
};

export const getFileMetadata = async (fileId: string) => {
  const response = await apiFetch(
    `/api/files/${encodeURIComponent(z.uuid().parse(fileId))}`,
  );
  if (!response.ok) throw await responseError(response);
  return fileMetadataSchema.parse(await response.json());
};

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
    const response = await apiFetch(path, { method: 'POST', body: data });
    if (!response.ok) throw await responseError(response);
    onProgress?.(100);
    return fileUploadResultSchema.parse(await response.json());
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
      request.onerror = () => reject(new ApiRequestError(0, 'Upload failed'));
      request.onload = () => {
        const body: unknown = (() => {
          try {
            return JSON.parse(request.responseText);
          } catch {
            return undefined;
          }
        })();
        if (request.status < 200 || request.status >= 300) {
          const error = apiErrorSchema.safeParse(body);
          reject(
            new ApiRequestError(
              request.status,
              error.success
                ? error.data.error.message
                : `Request failed (${request.status})`,
              error.success ? error.data.error.code : undefined,
            ),
          );
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
  const response = await apiFetch(
    `/api/agent/conversations/${encodeURIComponent(z.uuid().parse(conversationId))}/uploads`,
    { method: 'POST', body: data },
  );
  if (!response.ok) throw await responseError(response);
  return conversationUploadResultSchema.parse(await response.json());
};
