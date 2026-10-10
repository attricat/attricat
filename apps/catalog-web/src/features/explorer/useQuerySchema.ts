import { useQueries, useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
  getBlueprintByCode,
  type Attribute,
  type Blueprint,
  type BlueprintWithAttributes,
} from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import type { QuerySchema } from './queryLanguage';
import { queryTargetCodes } from './querySuggestions';
import { lexiconText, useLexiconRevision } from '../lexicon/lexicon';

const targetQueryKey = (code: string) =>
  recordQueryKeys.blueprintByCode(code, undefined);

/**
 * Builds the schema used to highlight and complete an Explorer query,
 * loading the current version of each relationship target the query walks
 * through, including the partial path being typed.
 */
export const useQuerySchema = (
  selected: BlueprintWithAttributes | undefined,
  blueprints: Blueprint[],
  query: string,
) => {
  const queryClient = useQueryClient();
  const base = useMemo(
    () =>
      selected && {
        blueprint: selected.blueprint,
        attributes: selected.attributes,
      },
    [selected],
  );
  // Follow paths through targets that are already cached before rendering,
  // so deeper hops are requested in the same pass.
  const codes: string[] = [];
  const cachedTargets = new Map<string, Attribute[]>();
  if (base) {
    for (;;) {
      const next = queryTargetCodes({ ...base, targets: cachedTargets }, query);
      let added = false;
      for (const code of next) {
        if (codes.includes(code)) continue;
        codes.push(code);
        added = true;
        const cached = queryClient.getQueryData<BlueprintWithAttributes>(
          targetQueryKey(code),
        );
        if (cached) cachedTargets.set(code, cached.attributes);
      }
      if (!added) break;
    }
  }
  const results = useQueries({
    queries: codes.map((code) => ({
      queryKey: targetQueryKey(code),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getBlueprintByCode(code, undefined, signal),
    })),
  });
  const targets = new Map(cachedTargets);
  results.forEach((result, index) => {
    if (result.data) targets.set(codes[index], result.data.attributes);
  });
  const schema: QuerySchema | undefined = base && { ...base, targets };
  const lexiconRevision = useLexiconRevision();
  const blueprintNames = useMemo(
    () =>
      new Map(
        blueprints.map((blueprint) => [
          blueprint.code,
          lexiconText(blueprint.name),
        ]),
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- names re-resolve when the lexicon changes
    [blueprints, lexiconRevision],
  );
  const targetsLoading = results.some((result) => result.isFetching);
  return { schema, blueprintNames, targetsLoading };
};
