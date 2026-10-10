// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { getRecordLabels } from '../api';
import { RecordHeading } from './RecordHeading';

vi.mock('../api', () => ({ getRecordLabels: vi.fn() }));

const recordId = '123e4567-e89b-12d3-a456-426614174001';

const renderHeading = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RecordHeading attributes={[]} recordId={recordId} values={{}} />
    </QueryClientProvider>,
  );

describe('RecordHeading without a heading component', () => {
  it('titles the record by its display label', async () => {
    vi.mocked(getRecordLabels).mockResolvedValueOnce({
      items: [
        {
          id: recordId,
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

  it('falls back to the ID when the record has no label', async () => {
    vi.mocked(getRecordLabels).mockResolvedValueOnce({ items: [] });
    renderHeading();
    expect(
      await screen.findByRole('heading', { level: 1, name: recordId }),
    ).toBeTruthy();
  });
});
