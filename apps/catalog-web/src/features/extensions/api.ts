import type { z } from 'zod';
import { request, requestText } from '../../api/request';
import { maximumExtensionResponseBytes } from './constants';
import {
  extensionCommandRequestSchema,
  type ExtensionStorageRequest,
  runtimeSchema,
} from './schemas';

export {
  extensionCommandRequestSchema,
  extensionStorageRequestSchema,
  type ExtensionContribution,
  type ExtensionStorageRequest,
  runtimeSchema,
} from './schemas';

export type ExtensionRuntimeScope = {
  blueprintId: string;
  blueprintVersion: number;
};

export const getExtensionRuntime = (scope?: ExtensionRuntimeScope) => {
  const query = scope
    ? `?blueprint_id=${encodeURIComponent(scope.blueprintId)}&blueprint_version=${scope.blueprintVersion}`
    : '';
  return request(`/api/extensions/runtime${query}`, runtimeSchema);
};

export const getExtensionArtifact = async (
  extensionId: string,
  contributionId: string,
) =>
  requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/artifact`,
  );

const parseExtensionResponse = (
  text: string,
  operation: 'command' | 'storage',
): unknown => {
  if (new TextEncoder().encode(text).length > maximumExtensionResponseBytes)
    throw new Error(`Extension ${operation} response is too large`);
  try {
    return JSON.parse(text) as unknown;
  } catch (error) {
    throw new Error(`Extension ${operation} response contains malformed JSON`, {
      cause: error,
    });
  }
};

export const extensionCommand = async (
  extensionId: string,
  contributionId: string,
  input: z.infer<typeof extensionCommandRequestSchema>,
) => {
  const response = await requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/command`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  return parseExtensionResponse(response, 'command');
};

export const extensionStorage = async (
  extensionId: string,
  contributionId: string,
  releaseId: string,
  input: ExtensionStorageRequest,
) => {
  const response = await requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/storage/${encodeURIComponent(releaseId)}`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  return parseExtensionResponse(response, 'storage');
};
