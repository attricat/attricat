import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import { extensionRunsPath } from './constants';
import {
  extensionRunDetailSchema,
  extensionRunSchema,
  startedExtensionRunSchema,
} from './schemas';

export type StartExtensionRunInput = {
  release_id: string;
  operation_id: string;
  input: Record<string, unknown>;
  idempotency_key: string;
  selection: {
    blueprint_id: string;
    blueprint_version: number;
    context_id: string | null;
    entity_ids: string[];
  };
};

/**
 * `own` limits a request to runs the signed-in user started, even for an
 * operator who may see every run. Extension frames always use it.
 */
export type ExtensionRunScope = 'visible' | 'own';

const runPath = (runId: string) =>
  `${extensionRunsPath}/${encodeURIComponent(z.uuid().parse(runId))}`;

const scoped = (path: string, scope: ExtensionRunScope) =>
  scope === 'own' ? `${path}?scope=own` : path;

/** Starts an interactive operation from a selection-aware contribution. */
export const startExtensionRun = (
  extensionId: string,
  contributionId: string,
  input: StartExtensionRunInput,
) =>
  request(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/operations`,
    startedExtensionRunSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    },
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
  scope: ExtensionRunScope = 'visible',
) => request(scoped(runPath(runId), scope), extensionRunDetailSchema);

export const cancelExtensionRun = (
  runId: string,
  scope: ExtensionRunScope = 'visible',
) =>
  requestNoContent(scoped(`${runPath(runId)}/cancel`, scope), {
    method: 'POST',
  });

export const extensionRunArtifactUrl = (
  runId: string,
  artifactId: string,
  scope: ExtensionRunScope = 'visible',
) =>
  scoped(
    `${runPath(runId)}/artifacts/${encodeURIComponent(z.uuid().parse(artifactId))}/download`,
    scope,
  );
