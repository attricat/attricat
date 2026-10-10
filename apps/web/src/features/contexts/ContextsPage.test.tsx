// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from './api';
import { ContextsPage } from './ContextsPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('./api', () => ({ listContexts: vi.fn() }));

it('labels parent contexts and preserves metadata for both known and missing parents', async () => {
  vi.mocked(listContexts).mockResolvedValue([
    { id: 'root', code: 'default', parent_id: null, data: {} },
    { id: 'child', code: 'local', parent_id: 'root', data: { region: 'east' } },
    { id: 'orphan', code: 'orphan', parent_id: 'missing', data: {} },
  ]);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <ContextsPage />
    </QueryClientProvider>,
  );
  expect(
    await screen.findByText('Parent: default · {"region":"east"}'),
  ).toBeTruthy();
  expect(screen.getByText('Parent: unknown · {}')).toBeTruthy();
  expect(screen.getByText('Root · {}')).toBeTruthy();
});
