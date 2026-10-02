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

const runPath = (runId: string) =>
  `${extensionRunsPath}/${encodeURIComponent(z.uuid().parse(runId))}`;

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

export const getExtensionRun = (runId: string) =>
  request(runPath(runId), extensionRunDetailSchema);

export const cancelExtensionRun = (runId: string) =>
  requestNoContent(`${runPath(runId)}/cancel`, { method: 'POST' });

export const extensionRunArtifactUrl = (runId: string, artifactId: string) =>
  `${runPath(runId)}/artifacts/${encodeURIComponent(z.uuid().parse(artifactId))}/download`;
