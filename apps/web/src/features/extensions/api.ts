import type { z } from 'zod';
import i18n from '../../i18n';
import { jsonRequest, request, requestText } from '../../api/request';
import { maximumExtensionResponseBytes } from './constants';
import {
  extensionCommandRequestSchema,
  type ExtensionStorageRequest,
  formAssistAnswerSchema,
  formAssistsResponseSchema,
  runtimeSchema,
} from './schemas';
import { utf8ByteLength } from './utf8';
import { extensionsApiPath, contributionPath } from './apiPaths';

export {
  extensionCommandRequestSchema,
  extensionStorageRequestSchema,
  type ExtensionContribution,
} from './schemas';

export type ExtensionRuntimeScope = {
  blueprintId: string;
  blueprintVersion: number;
};

export const getExtensionRuntime = (scope?: ExtensionRuntimeScope) => {
  const query = scope
    ? `?blueprint_id=${encodeURIComponent(scope.blueprintId)}&blueprint_version=${scope.blueprintVersion}`
    : '';
  return request(`${extensionsApiPath}/runtime${query}`, runtimeSchema);
};

/** The form assists a blueprint revision's record form runs. */
export const getFormAssists = (
  scope: ExtensionRuntimeScope,
  signal?: AbortSignal,
) =>
  request(
    `/api/v1/records/form-assists?blueprint_id=${encodeURIComponent(scope.blueprintId)}&blueprint_version=${scope.blueprintVersion}`,
    formAssistsResponseSchema,
    signal === undefined ? {} : { signal },
  );

/** Calls one form assist with the form's working values. */
export const runFormAssist = (
  input: {
    extension_id: string;
    id: string;
    blueprint_id: string;
    blueprint_version: number;
    record_id: string | null;
    context_id: string | null;
    values: Record<string, unknown>;
    relationships: Record<string, string[]>;
    edited: string[];
  },
  signal?: AbortSignal,
) =>
  request('/api/v1/records/form-assist', formAssistAnswerSchema, {
    ...jsonRequest('POST', input),
    ...(signal === undefined ? {} : { signal }),
  });

export const getExtensionArtifact = async (
  extensionId: string,
  contributionId: string,
) => requestText(`${contributionPath(extensionId, contributionId)}/artifact`);

const parseExtensionResponse = (
  text: string,
  operation: 'command' | 'storage',
): unknown => {
  if (utf8ByteLength(text) > maximumExtensionResponseBytes)
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
    `${contributionPath(extensionId, contributionId)}/command`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      // The host adds the viewer's language, so a 1.3 command can refuse in it.
      body: JSON.stringify({ ...input, locale: i18n.language }),
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
    `${contributionPath(extensionId, contributionId)}/storage/${encodeURIComponent(releaseId)}`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  return parseExtensionResponse(response, 'storage');
};
