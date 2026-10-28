import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { activeRunStatuses } from '../../api/runStatus';
import type { RuleRun } from './api';
import { ruleQueryKeys } from './queryKeys';

export const isActiveRun = (run: RuleRun) =>
  activeRunStatuses.includes(run.status);

/** The latest finish among the runs, which changes whenever a run finishes. */
export const latestCompletion = (runs: readonly RuleRun[] | undefined) =>
  runs?.reduce<string | null>(
    (latest, run) =>
      run.completed_at && (!latest || run.completed_at > latest)
        ? run.completed_at
        : latest,
    null,
  ) ?? null;

/**
 * Calls `onFinished` whenever a run in the loaded `runs` finishes after they
 * were first loaded, so what the runs change can be refreshed.
 */
export const useOnRunsFinished = (
  runs: readonly RuleRun[] | undefined,
  loaded: boolean,
  onFinished: () => void,
) => {
  const finishedAt = latestCompletion(runs);
  const seenFinish = useRef<string | null | undefined>(undefined);
  const callback = useRef(onFinished);
  useEffect(() => {
    callback.current = onFinished;
  });
  useEffect(() => {
    if (!loaded) return;
    if (seenFinish.current !== undefined && seenFinish.current !== finishedAt)
      callback.current();
    seenFinish.current = finishedAt;
  }, [finishedAt, loaded]);
};

/**
 * Refreshes every list and count of findings and the rules' finding counts
 * whenever one of the loaded runs finishes.
 */
export const useRefreshFindingsAfterRuns = (
  runs: readonly RuleRun[] | undefined,
  loaded: boolean,
) => {
  const client = useQueryClient();
  useOnRunsFinished(runs, loaded, () => {
    void Promise.all([
      client.invalidateQueries({ queryKey: ruleQueryKeys.findings() }),
      client.invalidateQueries({ queryKey: ruleQueryKeys.definitions() }),
    ]);
  });
};
