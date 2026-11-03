import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import {
  isNewerRelease,
  installedExtensionName,
  invalidateExtensions,
} from './extensionPageUtils';
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

it('names an installed extension by its manifest, or by its ID', () => {
  const extension_id = 'attricat-extension-example';
  expect(
    installedExtensionName({
      extension_id,
      manifest: { name: 'Attricat Example Extension' },
    }),
  ).toBe('Attricat Example Extension');
  for (const manifest of [{}, { name: ' ' }, null])
    expect(installedExtensionName({ extension_id, manifest })).toBe(
      extension_id,
    );
});

describe('isNewerRelease', () => {
  it.each([
    ['v1.3.0', '1.2.9', true],
    ['1.10.0', '1.9.0', true],
    ['v1.2.0', '1.2.0', false],
    ['v1.1.9', '1.2.0', false],
    ['1.2.0', '1.2.0-beta.1', true],
    ['1.2.0-beta.1', '1.2.0', false],
    ['1.2.0-beta.10', '1.2.0-beta.9', true],
    ['nightly', '1.2.0', true],
  ])('%s after %s: %s', (tag, installed, newer) => {
    expect(isNewerRelease(tag, installed)).toBe(newer);
  });
});
