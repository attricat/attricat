import { afterEach, describe, expect, it } from 'vitest';
import {
  clearTimingsForTest,
  recentTimings,
  recordFrameTiming,
  recordServerTiming,
} from './timing';

afterEach(clearTimingsForTest);

describe('development timing buffer', () => {
  it('keeps only fixed aggregate Server-Timing phases', () => {
    recordServerTiming(
      'candidate;dur=1.25, page;dur=2, sql;dur=8;desc=SELECT secret, related;dur=3, request-123;dur=4',
    );

    expect(recentTimings()).toEqual([
      {
        phases: [
          { name: 'candidate', duration: 1.25 },
          { name: 'page', duration: 2 },
          { name: 'related', duration: 3 },
        ],
        recordedAt: expect.any(Number),
      },
    ]);
    expect(JSON.stringify(recentTimings())).not.toContain('SELECT');
    expect(JSON.stringify(recentTimings())).not.toContain('request-123');
  });

  it('bounds entries and records frame outcomes without contribution data', () => {
    for (let index = 0; index < 55; index += 1)
      recordFrameTiming('frame-load', index);

    expect(recentTimings()).toHaveLength(50);
    expect(recentTimings()[0]?.phases).toEqual([
      { name: 'frame-load', duration: 54 },
    ]);
  });
});
