// @vitest-environment jsdom
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '../../i18n';
import { listBlueprintMigrationBatches } from './api';
import { MigrationBatchStatus } from './MigrationBatchStatus';

vi.mock('./api', () => ({ listBlueprintMigrationBatches: vi.fn() }));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';

const TestMigrationBatchStatus = () => {
  const batches = useQuery({
    queryKey: ['migration-batches'],
    queryFn: () => listBlueprintMigrationBatches(blueprintId),
  });
  return <MigrationBatchStatus batches={batches} />;
};

describe('MigrationBatchStatus', () => {
  afterEach(async () => {
    vi.clearAllMocks();
    await i18n.changeLanguage('en');
  });

  it('shows the persisted migration batch status and identifiers', async () => {
    vi.mocked(listBlueprintMigrationBatches).mockResolvedValue([
      {
        id: '223e4567-e89b-12d3-a456-426614174000',
        blueprint_id: blueprintId,
        target_version: 2,
        removal_policy: {},
        status: 'running',
        created_at: '2026-10-05T12:00:00Z',
        started_at: '2026-10-05T12:00:01Z',
        completed_at: null,
        total_records: 10_000,
        processed_records: 42,
        migrated_records: 40,
        needs_input_records: 1,
        failed_records: 1,
      },
    ]);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={client}>
        <TestMigrationBatchStatus />
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Running')).toBeDefined();
    expect(screen.getByText('v2')).toBeDefined();
    expect(screen.getByText('42 of 10000')).toBeDefined();
    expect(screen.getByText('40')).toBeDefined();
    expect(
      screen.getByText('223e4567-e89b-12d3-a456-426614174000'),
    ).toBeDefined();
    expect(listBlueprintMigrationBatches).toHaveBeenCalledWith(blueprintId);
  });
});
