import { QueryClient } from '@tanstack/react-query';
import { expect, it } from 'vitest';
import { invalidateExtensions } from './extensionPageUtils';
import { extensionQueryKeys } from './queryKeys';

it('invalidates every scoped and unscoped runtime descriptor', async () => {
  const client = new QueryClient();
  const scoped = {
    blueprintId: '22222222-2222-4222-8222-222222222222',
    blueprintVersion: 3,
  };
  client.setQueryData(extensionQueryKeys.runtime(), []);
  client.setQueryData(extensionQueryKeys.runtime(scoped), []);

  invalidateExtensions(client);
  await Promise.resolve();

  expect(
    client.getQueryState(extensionQueryKeys.runtime())?.isInvalidated,
  ).toBe(true);
  expect(
    client.getQueryState(extensionQueryKeys.runtime(scoped))?.isInvalidated,
  ).toBe(true);
});
