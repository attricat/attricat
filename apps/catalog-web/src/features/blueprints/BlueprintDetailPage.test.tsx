// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import '../../i18n';
import { blueprintQueryKeys } from './query-keys';
import { BlueprintDetailPage } from './BlueprintDetailPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  createLink: <T,>(component: T) => component,
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getBlueprintRevision: vi.fn(() => new Promise(() => {})),
  listBlueprintMigrationBatches: vi.fn(() => Promise.resolve([])),
  listBlueprintRevisions: vi.fn(() => Promise.resolve([])),
}));
vi.mock('../exports/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../exports/api')>()),
  listPublicationChannels: vi.fn(() => Promise.resolve([])),
}));

it('gives each blueprint detail instance independent tab IDs', () => {
  const client = new QueryClient();
  const id = '123e4567-e89b-12d3-a456-426614174000';
  client.setQueryData(blueprintQueryKeys.revisions(id), [
    {
      id,
      code: 'product',
      kind: 'entity',
      name: 'Product',
      version: 1,
      status: 'draft',
      updated_at: null,
    },
  ]);
  render(
    <QueryClientProvider client={client}>
      <BlueprintDetailPage blueprintId={id} />
      <BlueprintDetailPage blueprintId={id} />
    </QueryClientProvider>,
  );
  const tabs = screen.getAllByRole('tab');
  expect(tabs).toHaveLength(8);
  expect(new Set(tabs.map((tab) => tab.id)).size).toBe(tabs.length);
});
