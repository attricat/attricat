import { cleanup } from '@testing-library/react';
import { afterEach, beforeAll, vi } from 'vitest';
import i18n from 'i18next';

// Monaco's browser/worker runtime is covered by Playwright, not jsdom.
vi.mock('../components/monacoRuntime', () => ({}));

beforeAll(async () => {
  // Only tests that import the app's i18n module initialize translations.
  if (i18n.isInitializing) {
    await new Promise<void>((resolve) => {
      const initialized = () => {
        i18n.off('initialized', initialized);
        resolve();
      };
      i18n.on('initialized', initialized);
    });
  }
});

afterEach(() => {
  cleanup();
  try {
    // Persisted UI state, such as the Inspector's, must not leak between tests.
    localStorage.clear();
  } catch {
    // Some Node versions expose a localStorage that throws without a backing file.
  }
});
