import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { httpStatus } from './httpStatus';
import { retryUnlessRefused } from './queryRetries';
import { ApiRequestError } from './request';

describe('retryUnlessRefused', () => {
  it('does not retry a refused request or a malformed ID', () => {
    expect(
      retryUnlessRefused(0, new ApiRequestError(httpStatus.notFound, 'gone')),
    ).toBe(false);
    expect(
      retryUnlessRefused(0, z.uuid().safeParse('not-a-uuid').error as Error),
    ).toBe(false);
  });

  it('retries unexpected failures and rate limits a few times', () => {
    expect(retryUnlessRefused(0, new Error('offline'))).toBe(true);
    expect(
      retryUnlessRefused(
        0,
        new ApiRequestError(httpStatus.tooManyRequests, 'slow down'),
      ),
    ).toBe(true);
    expect(retryUnlessRefused(3, new Error('offline'))).toBe(false);
  });
});
