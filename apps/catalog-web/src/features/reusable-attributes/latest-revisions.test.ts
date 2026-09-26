import { describe, expect, it } from 'vitest';
import type { ReusableAttribute } from './api';
import { latestReusableAttributeRevisions } from './latest-revisions';

const revision = (definition_id: string, version: number, id: string) =>
  ({ definition_id, version, id }) as ReusableAttribute;

describe('latestReusableAttributeRevisions', () => {
  it('selects the highest version per definition regardless of input order', () => {
    const result = latestReusableAttributeRevisions([
      revision('a', 3, 'a3'),
      revision('b', 1, 'b1'),
      revision('a', 1, 'a1'),
      revision('b', 2, 'b2'),
    ]);
    expect(result.map(({ id }) => id)).toEqual(['a3', 'b2']);
  });
});
