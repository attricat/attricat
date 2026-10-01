import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

afterEach(() => {
  cleanup();
  try {
    // Persisted UI state, such as the Inspector's, must not leak between tests.
    localStorage.clear();
  } catch {
    // Some Node versions expose a localStorage that throws without a backing file.
  }
});
