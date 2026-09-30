import { afterEach, describe, expect, it, vi } from 'vitest';
import { INSPECTOR_ENABLED_STORAGE_KEY } from './constants';

const loadFlag = async (getItem: (key: string) => string | null) => {
  vi.stubGlobal('__CATALOG_DEVTOOLS__', false);
  vi.stubGlobal('localStorage', { getItem });
  vi.resetModules();
  return (await import('./enabled')).inspectorEnabled;
};

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('inspectorEnabled', () => {
  it('stays off without devtools or a stored opt-in', async () => {
    expect(await loadFlag(() => null)).toBe(false);
  });

  it('can be forced on through local storage', async () => {
    expect(
      await loadFlag((key) =>
        key === INSPECTOR_ENABLED_STORAGE_KEY ? 'true' : null,
      ),
    ).toBe(true);
  });

  it('stays off when local storage is unavailable', async () => {
    expect(
      await loadFlag(() => {
        throw new Error('blocked');
      }),
    ).toBe(false);
  });
});
