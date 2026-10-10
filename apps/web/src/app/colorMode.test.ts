// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useColorMode } from './colorMode';

afterEach(() => {
  vi.unstubAllGlobals();
  useColorMode.setState({ preference: null });
});

describe('color mode preference', () => {
  it('persists an explicit choice while keeping it in sync with the store', () => {
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      setItem: (key: string, value: string) => values.set(key, value),
    });

    useColorMode.getState().setPreference('dark');
    expect(useColorMode.getState().preference).toBe('dark');
    expect(values.get('attricat.color-mode')).toBe('dark');

    useColorMode.getState().setPreference('light');
    expect(useColorMode.getState().preference).toBe('light');
    expect(values.get('attricat.color-mode')).toBe('light');
  });
});
