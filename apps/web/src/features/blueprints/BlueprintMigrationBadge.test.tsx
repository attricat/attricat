// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listBlueprintMigrationBatches } from './api';
import { BlueprintMigrationBadge } from './BlueprintMigrationBadge';
import { migrationBatchStatuses } from './constants';
import type { BlueprintMigrationBatchStatus } from './schemas';

vi.mock('./api', () => ({ listBlueprintMigrationBatches: vi.fn() }));

vi.mock('../../components/RouterLink', () => ({
  RouterChip: ({
    label,
    params,
    search,
    to,
  }: {
    label: ReactNode;
    params: Record<string, string>;
    search: Record<string, string>;
    to: string;
  }) => (
    <a
      href={`${to.replace('$blueprintId', params.blueprintId!)}?tab=${search.tab}`}
    >
      {label}
    </a>
  ),
}));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';

const batch = (
  status: BlueprintMigrationBatchStatus['status'],
): BlueprintMigrationBatchStatus => ({
  id: '223e4567-e89b-12d3-a456-426614174000',
  blueprint_id: blueprintId,
  target_version: 2,
  removal_policy: {},
  status,
  created_at: '2026-10-05T12:00:00Z',
  started_at: '2026-10-05T12:00:01Z',
  completed_at: null,
  total_records: 100,
  processed_records: 42,
  migrated_records: 40,
  needs_input_records: 1,
  failed_records: 1,
});

const renderBadge = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <BlueprintMigrationBadge blueprintId={blueprintId} />
    </QueryClientProvider>,
  );

describe('BlueprintMigrationBadge', () => {
  afterEach(() => vi.clearAllMocks());

  it('links an active migration to the Migrations tab', async () => {
    vi.mocked(listBlueprintMigrationBatches).mockResolvedValue([
      batch(migrationBatchStatuses.running),
    ]);
    renderBadge();

    const link = await screen.findByRole('link', { name: /Migrating/ });
    expect(link.getAttribute('href')).toBe(
      `/manage/blueprints/${blueprintId}?tab=migrations`,
    );
  });

  it('stays hidden without an active migration', async () => {
    vi.mocked(listBlueprintMigrationBatches).mockResolvedValue([
      batch(migrationBatchStatuses.completed),
    ]);
    renderBadge();

    await vi.waitFor(() =>
      expect(listBlueprintMigrationBatches).toHaveBeenCalledWith(blueprintId),
    );
    expect(screen.queryByRole('link')).toBeNull();
  });
});
