import type { QueryClient } from '@tanstack/react-query';
import { listBlueprints } from '../blueprints/api';
import { blueprintQueryKeys } from '../blueprints/queryKeys';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/queryKeys';
import { getBlueprintByCode } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import { listRoles } from '../workspace/api';
import { workspaceQueryKeys } from '../workspace/queryKeys';
import { referenceStaleTimeMs } from './constants';

export type ReferenceBlueprint = {
  code: string;
  kind: string;
  name: string;
  version: number;
};

export type ReferenceAttribute = {
  code: string;
  targetBlueprint?: string;
  valueType: string;
};

/**
 * Workspace data that definition fields refer to. Lookups are best-effort:
 * a failed or forbidden request yields no suggestions rather than an error.
 */
export type DefinitionReferences = {
  blueprintAttributes: (
    code: string,
    version?: number,
  ) => Promise<ReferenceAttribute[]>;
  blueprints: () => Promise<ReferenceBlueprint[]>;
  contexts: () => Promise<string[]>;
  roles: () => Promise<string[]>;
};

const orEmpty = async <T>(lookup: () => Promise<T[]>) => {
  try {
    return await lookup();
  } catch {
    return [];
  }
};

export const createDefinitionReferences = (
  queryClient: QueryClient,
): DefinitionReferences => ({
  blueprintAttributes: (code, version) =>
    orEmpty(async () => {
      const blueprint = await queryClient.fetchQuery({
        queryFn: ({ signal }) => getBlueprintByCode(code, version, signal),
        queryKey: recordQueryKeys.blueprintByCode(code, version),
        staleTime: referenceStaleTimeMs,
      });
      return blueprint.attributes.map((attribute) => ({
        code: attribute.code,
        targetBlueprint: attribute.target_blueprint_code ?? undefined,
        valueType: attribute.value_type,
      }));
    }),
  blueprints: () =>
    orEmpty(() =>
      queryClient.fetchQuery({
        queryFn: listBlueprints,
        queryKey: blueprintQueryKeys.catalogue(),
        staleTime: referenceStaleTimeMs,
      }),
    ),
  contexts: () =>
    orEmpty(async () =>
      (
        await queryClient.fetchQuery({
          queryFn: ({ signal }) => listContexts(signal),
          queryKey: contextQueryKeys.all(),
          staleTime: referenceStaleTimeMs,
        })
      ).map((context) => context.code),
    ),
  roles: () =>
    orEmpty(async () =>
      (
        await queryClient.fetchQuery({
          queryFn: listRoles,
          queryKey: workspaceQueryKeys.roles(),
          staleTime: referenceStaleTimeMs,
        })
      ).map((role) => role.code),
    ),
});
