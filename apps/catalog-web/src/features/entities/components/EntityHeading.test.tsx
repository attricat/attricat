// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { getEntityLabels } from '../api';
import { EntityHeading } from './EntityHeading';

vi.mock('../api', () => ({ getEntityLabels: vi.fn() }));

const entityId = '123e4567-e89b-12d3-a456-426614174001';

const renderHeading = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <EntityHeading attributes={[]} entityId={entityId} values={{}} />
    </QueryClientProvider>,
  );

describe('EntityHeading without a heading component', () => {
  it('titles the entity by its display label', async () => {
    vi.mocked(getEntityLabels).mockResolvedValueOnce({
      items: [
        {
          id: entityId,
          blueprint_code: 'product',
          display: { default: 'Red shirt' },
        },
      ],
    });
    renderHeading();
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Red shirt' }),
    ).toBeTruthy();
  });

  it('falls back to the ID when the entity has no label', async () => {
    vi.mocked(getEntityLabels).mockResolvedValueOnce({ items: [] });
    renderHeading();
    expect(
      await screen.findByRole('heading', { level: 1, name: entityId }),
    ).toBeTruthy();
  });
});
