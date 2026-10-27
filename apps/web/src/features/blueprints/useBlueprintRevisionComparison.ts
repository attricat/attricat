import { blueprintRevisionOptions } from './queryOptions';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import type { Blueprint } from './schemas';

/**
 * Tracks the two revisions selected for comparison. Until the user chooses,
 * the previous revision is compared with the latest one.
 */
export const useBlueprintRevisionComparison = (
  blueprintId: string,
  revisions: readonly Blueprint[],
) => {
  const [leftSelection, setLeftSelection] = useState<number | null>(null);
  const [rightSelection, setRightSelection] = useState<number | null>(null);
  const leftVersion =
    leftSelection ?? revisions[1]?.version ?? revisions[0]?.version;
  const rightVersion = rightSelection ?? revisions[0]?.version ?? leftVersion;
  const left = useQuery(blueprintRevisionOptions(blueprintId, leftVersion));
  const right = useQuery(blueprintRevisionOptions(blueprintId, rightVersion));

  return {
    left,
    leftVersion,
    right,
    rightVersion,
    selectLeftVersion: setLeftSelection,
    selectRightVersion: setRightSelection,
  };
};

export type BlueprintRevisionComparison = ReturnType<
  typeof useBlueprintRevisionComparison
>;
