import { z } from 'zod';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import { extensionRunScopes, extensionRunsPath } from './constants';
import {
  extensionRunDetailSchema,
  extensionRunSchema,
  startedExtensionRunSchema,
} from './schemas';
import { uuidSegment } from '../../api/pathSegments';

type StartExtensionRunInput = {
  release_id: string;
  operation_id: string;
  input: Record<string, unknown>;
  idempotency_key: string;
  selection: {
    blueprint_id: string;
    blueprint_version: number;
    context_id: string | null;
    record_ids: string[];
  };
};

/**
 * `own` limits a request to runs the signed-in user started, even for an
 * operator who may see every run. Extension frames always use it.
 */
type ExtensionRunScope =
  (typeof extensionRunScopes)[keyof typeof extensionRunScopes];

const runPath = (runId: string) => `${extensionRunsPath}/${uuidSegment(runId)}`;

const scoped = (path: string, scope: ExtensionRunScope) =>
  scope === extensionRunScopes.own ? `${path}?scope=${scope}` : path;

/** Starts an interactive operation from a selection-aware contribution. */
export const startExtensionRun = (
  extensionId: string,
  contributionId: string,
  input: StartExtensionRunInput,
) =>
  request(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/operations`,
    startedExtensionRunSchema,
    jsonRequest('POST', input),
  );

export const listExtensionRuns = (extensionId?: string) =>
  request(
    extensionId
      ? `${extensionRunsPath}?extension_id=${encodeURIComponent(extensionId)}`
      : extensionRunsPath,
    z.array(extensionRunSchema),
  );

export const getExtensionRun = (
  runId: string,
  scope: ExtensionRunScope = extensionRunScopes.visible,
) => request(scoped(runPath(runId), scope), extensionRunDetailSchema);

export const cancelExtensionRun = (
  runId: string,
  scope: ExtensionRunScope = extensionRunScopes.visible,
) =>
  requestNoContent(scoped(`${runPath(runId)}/cancel`, scope), {
    method: 'POST',
  });

export const extensionRunArtifactUrl = (
  runId: string,
  artifactId: string,
  scope: ExtensionRunScope = extensionRunScopes.visible,
) =>
  scoped(
    `${runPath(runId)}/artifacts/${uuidSegment(artifactId)}/download`,
    scope,
  );
