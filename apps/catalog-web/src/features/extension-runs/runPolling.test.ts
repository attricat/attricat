import { describe, expect, it } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { extensionRunPollMilliseconds } from './constants';
import {
  extensionRunRefetchInterval,
  isActiveExtensionRun,
} from './runPolling';

describe('extension run polling', () => {
  it('treats queued, running and cancelling runs as active', () => {
    expect(isActiveExtensionRun({ status: 'queued' })).toBe(true);
    expect(isActiveExtensionRun({ status: 'running' })).toBe(true);
    expect(isActiveExtensionRun({ status: 'cancelling' })).toBe(true);
    expect(isActiveExtensionRun({ status: 'completed' })).toBe(false);
    expect(isActiveExtensionRun({ status: 'failed' })).toBe(false);
    expect(isActiveExtensionRun({ status: 'cancelled' })).toBe(false);
  });

  it('polls active runs until a client error', () => {
    expect(extensionRunRefetchInterval(true, null)).toBe(
      extensionRunPollMilliseconds,
    );
    expect(extensionRunRefetchInterval(false, null)).toBe(false);
    expect(
      extensionRunRefetchInterval(true, new ApiRequestError(404, 'Not found')),
    ).toBe(false);
    // Server and network failures may recover, so polling continues.
    expect(
      extensionRunRefetchInterval(true, new ApiRequestError(503, 'Busy')),
    ).toBe(extensionRunPollMilliseconds);
    expect(extensionRunRefetchInterval(true, new Error('offline'))).toBe(
      extensionRunPollMilliseconds,
    );
  });
});
