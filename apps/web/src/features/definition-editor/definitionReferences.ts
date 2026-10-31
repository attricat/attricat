import type { QueryClient } from '@tanstack/react-query';
import { referenceStaleTimeMs } from './constants';
import { blueprintByCodeOptions } from '../records/queryOptions';
import { contextListOptions } from '../contexts/queryOptions';
import { blueprintCatalogueOptions } from '../blueprints/queryOptions';
import { roleListOptions } from '../workspace/queryOptions';
import { getExtensionRuntime } from '../extensions/api';
import { contributionKinds } from '../extensions/constants';
import { extensionQueryKeys } from '../extensions/queryKeys';

type ReferenceBlueprint = {
  code: string;
  id: string;
  kind: string;
  name: string;
  version: number;
};

/** An enabled extension's attribute editor a detail field may name. */
export type ReferenceEditor = {
  id: string;
  version: number;
  extensionName: string;
  title: string | null;
  valueTypes: string[];
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
  /** Attribute editors of enabled extensions. */
  attributeEditors?: () => Promise<ReferenceEditor[]>;
  /**
   * Attributes the definition refers to without declaring them, such as the
   * blueprint revision a rule checks.
   */
  scopeAttributes?: readonly ReferenceAttribute[];
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
        ...blueprintByCodeOptions(code, version),
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
        ...blueprintCatalogueOptions(),
        staleTime: referenceStaleTimeMs,
      }),
    ),
  contexts: () =>
    orEmpty(async () =>
      (
        await queryClient.fetchQuery({
          ...contextListOptions(),
          staleTime: referenceStaleTimeMs,
        })
      ).map((context) => context.code),
    ),
  roles: () =>
    orEmpty(async () =>
      (
        await queryClient.fetchQuery({
          ...roleListOptions(),
          staleTime: referenceStaleTimeMs,
        })
      ).map((role) => role.code),
    ),
  attributeEditors: () =>
    orEmpty(async () =>
      (
        await queryClient.fetchQuery({
          queryKey: extensionQueryKeys.runtime(),
          queryFn: () => getExtensionRuntime(),
          staleTime: referenceStaleTimeMs,
        })
      ).flatMap((contribution) =>
        contribution.kind === contributionKinds.editor && contribution.editor
          ? [
              {
                id: contribution.id,
                version: contribution.version,
                extensionName: contribution.extension_name,
                title: contribution.title,
                valueTypes: contribution.editor.value_types,
              },
            ]
          : [],
      ),
    ),
});
