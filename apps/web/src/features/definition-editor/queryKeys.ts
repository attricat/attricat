import type { DefinitionKind } from './constants';

export const definitionQueryKeys = {
  all: ['definition-check'] as const,
  check: (
    kind: DefinitionKind,
    resourceId: string | null | undefined,
    definition: string,
  ) =>
    [...definitionQueryKeys.all, kind, resourceId ?? null, definition] as const,
};
