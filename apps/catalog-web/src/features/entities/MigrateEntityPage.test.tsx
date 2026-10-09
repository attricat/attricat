// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from '../contexts/api';
import { migrateEntity, previewEntityMigration } from './api';
import { MigrateEntityPage } from './MigrateEntityPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('./api', () => ({
  migrateEntity: vi.fn(),
  previewEntityMigration: vi.fn(),
}));
vi.mock('./components/EntityForm', () => ({
  EntityForm: ({
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
  vi.mocked(previewEntityMigration).mockResolvedValue({
    migration_id: 'migration-1',
    source_version: 1,
    target: {
      blueprint: { id: 'blueprint-1', name: 'Product', version: 2 },
      attributes: [],
    },
    values: [],
    status: 'needs_input',
    issues: [{ kind: 'removed', attribute_code: 'legacy', message: 'Removed' }],
  } as unknown as Awaited<ReturnType<typeof previewEntityMigration>>);
  vi.mocked(migrateEntity).mockImplementation(() => new Promise(() => {}));
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MigrateEntityPage entityId="entity-1" />
    </QueryClientProvider>,
  );
  const user = userEvent.setup();
  const confirmation = await screen.findByRole('checkbox', {
    name: 'Confirm removal of legacy from this revision',
  });
  await user.click(confirmation);
  await user.click(screen.getByRole('button', { name: 'Submit migration' }));
  expect(migrateEntity).toHaveBeenCalledWith(
    'entity-1',
    expect.objectContaining({
      discard_attributes: ['legacy'],
    }),
  );
  expect((confirmation as HTMLInputElement).disabled).toBe(true);
});
