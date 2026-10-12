import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { checkDelayMs, type DefinitionKind } from './constants';

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
  kind,
  resourceId,
  validate,
}: {
  definition: string;
  kind: DefinitionKind;
  /** The saved blueprint or attribute the definition revises, if any. */
  resourceId?: string;
  validate: (
    definition: string,
    resourceId: string | undefined,
    signal: AbortSignal,
  ) => Promise<T>;
}): DefinitionCheck<T> => {
  const [checked, setChecked] = useState(definition);
  useEffect(() => {
    const timeout = window.setTimeout(
      () => setChecked(definition),
      checkDelayMs,
    );
    return () => window.clearTimeout(timeout);
  }, [definition]);
  const empty = !checked.trim();
  const query = useQuery({
    enabled: !empty,
    gcTime: 0,
    placeholderData: keepPreviousData,
    queryFn: ({ signal }) => validate(checked, resourceId, signal),
    queryKey: ['definition-check', kind, resourceId ?? null, checked],
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
  return {
    error: status === checkStatuses.invalid ? query.error?.message : undefined,
    result: lastValid?.result,
    stale: lastValid !== undefined && lastValid.definition !== definition,
    status,
  };
};
