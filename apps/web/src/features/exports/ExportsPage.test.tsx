// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ExportsPage } from './ExportsPage';

const { canEdit } = vi.hoisted(() => ({ canEdit: { value: false } }));

vi.mock(
  '@tanstack/react-router',
  async () => (await import('../../test/routerLinkMock')).routerLinkMock,
);
vi.mock('../auth/useCan', () => ({
  useAccess: () => ({ can: () => canEdit.value, isPending: false }),
  useCan: () => false,
}));
vi.mock('../contexts/api', () => ({
  listContexts: async () => [
    { id: '00000000-0000-4000-8000-000000000011', code: 'web' },
  ],
}));
vi.mock('./api', () => ({
  listPublicationChannels: async () => [],
  updatePublicationChannel: vi.fn(),
}));

const renderPage = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ExportsPage />
    </QueryClientProvider>,
  );

describe('ExportsPage', () => {
  it('tells readers that the channels are read-only', async () => {
    canEdit.value = false;
    renderPage();
    expect(
      await screen.findByText(/You can view export channels but not change/),
    ).toBeTruthy();
  });

  it('says nothing about access to someone who can change them', async () => {
    canEdit.value = true;
    renderPage();
    expect(await screen.findByText('web')).toBeTruthy();
    expect(
      screen.queryByText(/You can view export channels but not change/),
    ).toBeNull();
  });
});
