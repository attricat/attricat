// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from '../contexts/api';
import { migrateRecord, previewRecordMigration } from './api';
import { MigrateRecordPage } from './MigrateRecordPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('./api', () => ({
  migrateRecord: vi.fn(),
  previewRecordMigration: vi.fn(),
}));
vi.mock('./components/RecordForm', () => ({
  RecordForm: ({
    onSubmit,
  }: {
    onSubmit: (values: { values: []; relationships: [] }) => void;
  }) => (
    <button
      onClick={() => onSubmit({ values: [], relationships: [] })}
      type="button"
    >
      Submit migration
    </button>
  ),
}));

it('freezes removal confirmations and submits their snapshot while migrating', async () => {
  vi.mocked(listContexts).mockResolvedValue([]);
  vi.mocked(previewRecordMigration).mockResolvedValue({
    migration_id: 'migration-1',
    source_version: 1,
    target: {
      blueprint: { id: 'blueprint-1', name: 'Product', version: 2 },
      attributes: [],
    },
    values: [],
    status: 'needs_input',
    issues: [{ kind: 'removed', attribute_code: 'legacy', message: 'Removed' }],
  } as unknown as Awaited<ReturnType<typeof previewRecordMigration>>);
  vi.mocked(migrateRecord).mockImplementation(() => new Promise(() => {}));
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MigrateRecordPage recordId="record-1" />
    </QueryClientProvider>,
  );
  const user = userEvent.setup();
  const confirmation = await screen.findByRole('checkbox', {
    name: 'Confirm removal of legacy from this revision',
  });
  await user.click(confirmation);
  await user.click(screen.getByRole('button', { name: 'Submit migration' }));
  expect(migrateRecord).toHaveBeenCalledWith(
    'record-1',
    expect.objectContaining({
      discard_attributes: ['legacy'],
    }),
  );
  expect((confirmation as HTMLInputElement).disabled).toBe(true);
});
