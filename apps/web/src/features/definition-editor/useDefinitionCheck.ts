import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { checkDelayMs, type DefinitionKind } from './constants';
import { definitionQueryKeys } from './queryKeys';
import { errorMessage } from '../../api/errors';
import { useDebouncedValue } from '../../components/useDebouncedValue';

export const checkStatuses = {
  checking: 'checking',
  idle: 'idle',
  invalid: 'invalid',
  valid: 'valid',
} as const;
export type CheckStatus = (typeof checkStatuses)[keyof typeof checkStatuses];

export type DefinitionCheck<T> = {
  status: CheckStatus;
  /** The server's error for the current definition, while it is invalid. */
  error?: string;
  /**
   * The server's untranslated message for `error`, which names what it rejects
   * the same way in every language; only for finding where the error points.
   */
  serverMessage?: string;
  /** The most recent definition that passed, for the preview. */
  result?: T;
  /** Whether `result` comes from an earlier version of the definition. */
  stale: boolean;
};

/**
 * Runs the server's save checks on a definition shortly after typing stops,
 * without storing it. Keeps the last valid result so a preview can stay on
 * screen while the definition is briefly invalid.
 */
export const useDefinitionCheck = <T>({
  definition,
  enabled = true,
  kind,
  resourceId,
  scope,
  validate,
}: {
  definition: string;
  /** False while the definition cannot be checked yet; the check is idle. */
  enabled?: boolean;
  kind: DefinitionKind;
  /** The saved resource the definition revises, if any. */
  resourceId?: string;
  /**
   * Identifies anything else `validate` checks the definition against, such
   * as a rule's blueprint revision, so a change re-runs the check.
   */
  scope?: string;
  validate: (
    definition: string,
    resourceId: string | undefined,
    signal: AbortSignal,
  ) => Promise<T>;
}): DefinitionCheck<T> => {
  const checked = useDebouncedValue(definition, checkDelayMs);
  const empty = !enabled || !checked.trim();
  const query = useQuery({
    enabled: !empty,
    gcTime: 0,
    placeholderData: keepPreviousData,
    queryFn: ({ signal }) => validate(checked, resourceId, signal),
    queryKey: definitionQueryKeys.check(kind, resourceId, checked, scope),
    retry: false,
  });
  const [lastValid, setLastValid] = useState<{
    definition: string;
    result: T;
  }>();
  if (
    query.isSuccess &&
    !query.isPlaceholderData &&
    lastValid?.result !== query.data
  )
    setLastValid({ definition: checked, result: query.data });

  const pending =
    checked !== definition || query.isFetching || query.isPlaceholderData;
  const status: CheckStatus = empty
    ? checkStatuses.idle
    : pending
      ? checkStatuses.checking
      : query.isError
        ? checkStatuses.invalid
        : checkStatuses.valid;
  const failure = status === checkStatuses.invalid ? query.error : null;
  return {
    error: failure ? errorMessage(failure) : undefined,
    serverMessage: failure?.message,
    result: lastValid?.result,
    stale: lastValid !== undefined && lastValid.definition !== definition,
    status,
  };
};
