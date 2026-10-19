// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from '../contexts/api';
import {
  getBlueprintByCode,
  getRecordLabels,
  migrateRecord,
  previewRecordMigration,
} from './api';
import { MigrateRecordPage } from './MigrateRecordPage';

vi.mock('@tanstack/react-router', () => ({
  createLink:
    () =>
    ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('../contexts/api', () => ({ listContexts: vi.fn() }));
vi.mock('./api', () => ({
  getBlueprintByCode: vi.fn(),
  getRecordLabels: vi.fn(),
  migrateRecord: vi.fn(),
  previewRecordMigration: vi.fn(),
}));
// Stands in for the form: shows what the page passes and submits.
vi.mock('./components/RecordForm', () => ({
  RecordForm: ({
    children,
    migrationReviews,
    onSubmit,
    pendingConfirmations,
  }: {
    children: React.ReactNode;
    migrationReviews: Record<
      string,
      { message: string; previousValue?: string }
    >;
    onSubmit: (values: { values: []; relationships: [] }) => void;
    pendingConfirmations: { label: string }[];
  }) => (
    <>
      {children}
      <ul aria-label="Reviews">
        {Object.entries(migrationReviews).map(([code, review]) => (
          <li key={code}>
            {code}: {review.message} ({review.previousValue})
          </li>
        ))}
      </ul>
      <p>{pendingConfirmations.length} to confirm</p>
      <button
        onClick={() => onSubmit({ values: [], relationships: [] })}
        type="button"
      >
        Submit migration
      </button>
    </>
  ),
}));

it('freezes removal confirmations and submits their snapshot while migrating', async () => {
  vi.mocked(listContexts).mockResolvedValue([]);
  vi.mocked(getBlueprintByCode).mockResolvedValue({
    blueprint: {
      id: 'blueprint-1',
      code: 'product',
      name: 'Product',
      version: 1,
    },
    attributes: [{ code: 'legacy', name: 'Legacy code', value_type: 'string' }],
  } as unknown as Awaited<ReturnType<typeof getBlueprintByCode>>);
  vi.mocked(previewRecordMigration).mockResolvedValue({
    migration_id: 'migration-1',
    source_version: 1,
    target: {
      blueprint: {
        id: 'blueprint-1',
        code: 'product',
        name: 'Product',
        version: 2,
      },
      attributes: [],
    },
    values: [
      {
        kind: 'scalar',
        attribute_code: 'legacy',
        context_id: null,
        value: 'L-1',
      },
    ],
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
    name: 'Discard the stored Legacy code value',
  });
  expect(screen.getByText('Previous value: L-1')).toBeTruthy();
  expect(screen.getByText('1 to confirm')).toBeTruthy();
  await user.click(confirmation);
  expect(screen.getByText('0 to confirm')).toBeTruthy();
  await user.click(screen.getByRole('button', { name: 'Submit migration' }));
  expect(migrateRecord).toHaveBeenCalledWith(
    'record-1',
    expect.objectContaining({
      discard_attributes: ['legacy'],
    }),
  );
  expect((confirmation as HTMLInputElement).disabled).toBe(true);
});

it("explains inline issues in the user's language with the previous value", async () => {
  vi.mocked(listContexts).mockResolvedValue([]);
  vi.mocked(getBlueprintByCode).mockResolvedValue({
    blueprint: {
      id: 'blueprint-1',
      code: 'product',
      name: 'Product',
      version: 1,
    },
    attributes: [],
  } as unknown as Awaited<ReturnType<typeof getBlueprintByCode>>);
  vi.mocked(getRecordLabels).mockResolvedValue({
    items: [
      {
        id: '123e4567-e89b-12d3-a456-426614174099',
        blueprint_code: 'supplier',
        display: { default: 'Acme' },
      },
    ],
  });
  vi.mocked(previewRecordMigration).mockResolvedValue({
    migration_id: 'migration-1',
    source_version: 1,
    target: {
      blueprint: {
        id: 'blueprint-1',
        code: 'product',
        name: 'Product',
        version: 2,
      },
      attributes: [
        { code: 'supplier', value_type: 'relationship' },
        { code: 'size', value_type: 'number' },
      ],
    },
    values: [
      {
        kind: 'relationship',
        attribute_code: 'supplier',
        context_id: null,
        target_record_id: '123e4567-e89b-12d3-a456-426614174099',
      },
    ],
    status: 'needs_input',
    issues: [
      {
        kind: 'relationship_cardinality_changed',
        attribute_code: 'supplier',
        message: 'The relationship cardinality changed in the target revision',
      },
      { kind: 'future_kind', attribute_code: 'size', message: 'Server says' },
    ],
  } as unknown as Awaited<ReturnType<typeof previewRecordMigration>>);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <MigrateRecordPage recordId="record-1" />
    </QueryClientProvider>,
  );
  expect(
    await screen.findByText(
      'supplier: The number of records this field may link to changed. (Acme)',
    ),
  ).toBeTruthy();
  expect(screen.getByText('size: Server says ()')).toBeTruthy();
  expect(screen.getByText('2 fields to review')).toBeTruthy();
});
