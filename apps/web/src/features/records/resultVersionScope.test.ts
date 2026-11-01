import { describe, expect, it } from 'vitest';
import { resultVersionScopeKinds } from './constants';
import { resultsShowBlueprintRevision } from './schemas';

const page = (result_version_scope: never) => ({
  blueprint: { blueprint: { version: 3 } },
  result_version_scope,
});

describe('resultsShowBlueprintRevision', () => {
  it('offers revision actions when there are no results, so an import can start', () => {
    expect(
      resultsShowBlueprintRevision(
        page({ kind: resultVersionScopeKinds.empty } as never),
      ),
    ).toBe(true);
  });

  it('offers them only for results of the shown revision', () => {
    const single = (version: number) =>
      page({ kind: resultVersionScopeKinds.single, version } as never);
    expect(resultsShowBlueprintRevision(single(3))).toBe(true);
    expect(resultsShowBlueprintRevision(single(2))).toBe(false);
    expect(
      resultsShowBlueprintRevision(
        page({ kind: resultVersionScopeKinds.multiple } as never),
      ),
    ).toBe(false);
    expect(resultsShowBlueprintRevision(undefined)).toBe(false);
  });
});
