import { describe, expect, it } from 'vitest';
import { notFoundErrorCode } from './errors';
import { httpStatus } from './httpStatus';
import { uuidSegment } from './pathSegments';
import { ApiRequestError } from './request';

describe('uuidSegment', () => {
  it('keeps a UUID', () => {
    const id = '00000000-0000-4000-8000-000000000001';
    expect(uuidSegment(id)).toBe(id);
  });

  it('fails a malformed ID as not found', () => {
    expect.assertions(3);
    try {
      uuidSegment('not-an-id');
    } catch (error) {
      expect(error).toBeInstanceOf(ApiRequestError);
      expect((error as ApiRequestError).status).toBe(httpStatus.notFound);
      expect((error as ApiRequestError).code).toBe(notFoundErrorCode);
    }
  });
});
