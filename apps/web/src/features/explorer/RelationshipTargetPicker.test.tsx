// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { searchRecords } from '../records/api';
import { RelationshipTargetPicker } from './RelationshipTargetPicker';
import { relationshipPickerManyMessageType } from '../records/components/useRecentlyPreviewedRecords';

vi.mock('../records/api', () => ({
  getRecordLabels: vi.fn(() => new Promise(() => {})),
  searchRecords: vi.fn(),
}));

const sampleId = '123e4567-e89b-12d3-a456-426614174001';

describe('RelationshipTargetPicker', () => {
  it('marks sample targets and toggles the chosen ones', async () => {
    vi.mocked(searchRecords).mockResolvedValue({
      blueprint: { blueprint: { views: {} } },
      items: [
        {
          id: sampleId,
          display: { name: 'Sample chair' },
          is_sample: true,
          preview: {},
        },
      ],
      next_cursor: null,
    } as never);
    const onSelectedIdsChange = vi.fn();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RelationshipTargetPicker
          onSelectedIdsChange={onSelectedIdsChange}
          selectedIds={[sampleId]}
          targetBlueprints={['product']}
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Sample')).toBeTruthy();
    await userEvent.click(screen.getByRole('checkbox', { checked: true }));
    expect(onSelectedIdsChange).toHaveBeenCalledWith([]);
  });

  it('switches between the target blueprints of a relationship', async () => {
    vi.mocked(searchRecords).mockResolvedValue({
      blueprint: { blueprint: { views: {} } },
      items: [],
      next_cursor: null,
    } as never);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RelationshipTargetPicker
          blueprintName={(code) =>
            ({ product: 'Product', material: 'Material' })[code] ?? code
          }
          onSelectedIdsChange={vi.fn()}
          selectedIds={[]}
          targetBlueprints={['product', 'material']}
        />
      </QueryClientProvider>,
    );

    expect(vi.mocked(searchRecords)).toHaveBeenLastCalledWith(
      expect.objectContaining({ blueprint: 'product' }),
    );
    await userEvent.click(
      screen.getByRole('combobox', { name: 'Target blueprint' }),
    );
    await userEvent.click(screen.getByRole('option', { name: 'Material' }));
    expect(vi.mocked(searchRecords)).toHaveBeenLastCalledWith(
      expect.objectContaining({ blueprint: 'material' }),
    );
  });

  it('adds the records chosen in an Explorer tab it opened', async () => {
    vi.mocked(searchRecords).mockResolvedValue({
      blueprint: { blueprint: { views: {} } },
      items: [],
      next_cursor: null,
    } as never);
    const onSelectedIdsChange = vi.fn();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RelationshipTargetPicker
          onSelectedIdsChange={onSelectedIdsChange}
          selectedIds={[sampleId]}
          targetBlueprints={['product']}
        />
      </QueryClientProvider>,
    );

    const link = screen.getByRole('link', { name: 'Browse' });
    const href = new URL(link.getAttribute('href')!, window.location.origin);
    expect(href.searchParams.get('blueprint')).toBe('product');
    const token = href.searchParams.get('relationshipPicker');
    expect(token).toBeTruthy();
    expect(link.getAttribute('target')).toBe('_blank');

    const chosen = '123e4567-e89b-12d3-a456-426614174002';
    act(() => {
      window.dispatchEvent(
        new MessageEvent('message', {
          origin: window.location.origin,
          data: {
            type: relationshipPickerManyMessageType,
            token,
            recordIds: [sampleId, chosen],
            labels: {},
          },
        }),
      );
    });
    expect(onSelectedIdsChange).toHaveBeenCalledWith([sampleId, chosen]);
  });
});
