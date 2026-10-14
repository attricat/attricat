import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import { invalidateAuthorization } from './invalidateAuthorization';
import { authQueryKeys } from './queryKeys';

describe('invalidateAuthorization', () => {
  it('marks the session and every access check stale', async () => {
    const client = new QueryClient();
    const keys = [
      authQueryKeys.session(),
      authQueryKeys.access('records.delete', 'record', ['a']),
      authQueryKeys.access('contexts.write', 'context', ['b', 'c']),
    ];
    for (const key of keys) client.setQueryData(key, {});
    client.setQueryData(authQueryKeys.sampleLogins(), null);

    await invalidateAuthorization(client);

    for (const key of keys)
      expect(client.getQueryState(key)?.isInvalidated).toBe(true);
    expect(
      client.getQueryState(authQueryKeys.sampleLogins())?.isInvalidated,
    ).toBe(false);
  });
});
