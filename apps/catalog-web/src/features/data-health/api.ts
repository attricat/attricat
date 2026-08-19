import { z } from 'zod';
import {
  blueprintHealthSchema,
  completenessHealthSchema,
  contextHealthSchema,
  dataHealthSummarySchema,
  freshnessBandSchema,
  relationshipHealthSchema,
  storageHealthSchema,
} from './schemas';

const request = async <T>(path: string, schema: z.ZodType<T>): Promise<T> => {
  const response = await fetch(path);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  const result = schema.safeParse(await response.json());
  if (!result.success)
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  return result.data;
};

export const getDataHealthSummary = (staleAfterDays: number) =>
  request(
    `/api/data-health/summary?stale_after_days=${staleAfterDays}`,
    dataHealthSummarySchema,
  );
export const getDataHealthBlueprints = (staleAfterDays: number) =>
  request(
    `/api/data-health/blueprints?stale_after_days=${staleAfterDays}`,
    z.array(blueprintHealthSchema),
  );
export const getDataHealthFreshness = () =>
  request('/api/data-health/freshness', z.array(freshnessBandSchema));
export const getDataHealthCompleteness = () =>
  request('/api/data-health/completeness', z.array(completenessHealthSchema));
export const getDataHealthContexts = () =>
  request('/api/data-health/contexts', z.array(contextHealthSchema));
export const getDataHealthRelationships = () =>
  request('/api/data-health/relationships', z.array(relationshipHealthSchema));
export const getDataHealthStorage = () =>
  request('/api/data-health/storage', z.array(storageHealthSchema));

export const refreshDataHealth = async () => {
  const response = await fetch('/api/data-health/refresh', { method: 'POST' });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
