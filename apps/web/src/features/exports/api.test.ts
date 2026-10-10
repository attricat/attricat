import { afterEach, describe, expect, it, vi } from 'vitest';
import { listPublicationChannels, updatePublicationChannel } from './api';

const contextId = '123e4567-e89b-12d3-a456-426614174000';
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

describe('publication channel API client', () => {
  it('lists publication channels', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve([
          { context_id: contextId, context_code: 'storefront', enabled: true },
        ]),
    });

    await expect(listPublicationChannels()).resolves.toEqual([
      {
        context_id: contextId,
        context_code: 'storefront',
        enabled: true,
        required_rule_codes: [],
        require_valid_record: false,
      },
    ]);
    expect(fetchMock).toHaveBeenCalledWith('/api/publication-channels');
  });

  it('updates a channel state', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          context_id: contextId,
          context_code: 'storefront',
          enabled: false,
        }),
    });

    await updatePublicationChannel(contextId, false);

    expect(fetchMock).toHaveBeenCalledWith(
      `/api/publication-channels/${contextId}`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: false }),
      },
    );
  });

  it('updates the publication checks a channel requires', async () => {
    const channel = {
      context_id: contextId,
      context_code: 'storefront',
      enabled: true,
      required_rule_codes: ['has-sku'],
      require_valid_record: true,
    };
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(channel),
    });

    await expect(
      updatePublicationChannel(contextId, true, {
        required_rule_codes: ['has-sku'],
        require_valid_record: true,
      }),
    ).resolves.toEqual(channel);
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/publication-channels/${contextId}`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          enabled: true,
          required_rule_codes: ['has-sku'],
          require_valid_record: true,
        }),
      },
    );
  });
});
