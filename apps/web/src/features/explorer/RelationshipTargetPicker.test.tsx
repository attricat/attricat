// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { searchRecords } from '../records/api';
import { RelationshipTargetPicker } from './RelationshipTargetPicker';

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
          targetBlueprint="product"
        />
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Sample')).toBeTruthy();
    await userEvent.click(screen.getByRole('checkbox', { checked: true }));
    expect(onSelectedIdsChange).toHaveBeenCalledWith([]);
  });
});
