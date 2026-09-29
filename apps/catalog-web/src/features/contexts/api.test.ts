import { afterEach, describe, expect, it, vi } from 'vitest';
import { createContext, listContexts } from './api';

const contextId = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

describe('context API client', () => {
  it('loads contexts using the contexts endpoint', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve([
          { id: contextId, code: 'default', data: {}, parent_id: null },
        ]),
    });

    await expect(listContexts()).resolves.toEqual([
      { id: contextId, code: 'default', data: {}, parent_id: null },
    ]);
    expect(fetchMock).toHaveBeenCalledWith('/api/contexts');
  });

  it('forwards navigation cancellation signals', async () => {
    const controller = new AbortController();
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve([
          { id: contextId, code: 'default', data: {}, parent_id: null },
        ]),
    });

    await listContexts(controller.signal);

    expect(fetchMock).toHaveBeenCalledWith('/api/contexts', {
      signal: controller.signal,
    });
  });

  it('validates and posts context payloads', async () => {
    expect(() => createContext('en GB', {}, contextId)).toThrow('hyphens');
    expect(fetchMock).not.toHaveBeenCalled();

    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          id: contextId,
          code: 'en_GB',
          data: { locale: 'en-GB' },
          parent_id: contextId,
        }),
    });
    await createContext('en_GB', { locale: 'en-GB' }, contextId);

    expect(fetchMock).toHaveBeenLastCalledWith('/api/contexts', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        code: 'en_GB',
        data: { locale: 'en-GB' },
        parent_id: contextId,
      }),
    });
  });
});

describe('context code validation', () => {
  it('reports invalid codes in the active language', async () => {
    const { default: i18n } = await import('../../i18n');
    const invalid = () => createContext('not valid', {}, contextId);
    try {
      await i18n.changeLanguage('pl');
      expect(invalid).toThrow(
        'Używaj tylko liter, cyfr, łączników i podkreśleń',
      );
      await i18n.changeLanguage('en');
      expect(invalid).toThrow(
        'Use only letters, numbers, hyphens, and underscores',
      );
    } finally {
      await i18n.changeLanguage('en');
    }
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
