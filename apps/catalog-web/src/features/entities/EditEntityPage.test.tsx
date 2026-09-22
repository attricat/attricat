// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { getCurrentBlueprint, getEntityForm } from './api';
import { EditEntityPage } from './EditEntityPage';

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getCurrentBlueprint: vi.fn(),
  getEntityForm: vi.fn(),
  getResolvedEntityPreview: vi.fn(),
  updateEntity: vi.fn(),
}));
vi.mock('../contexts/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../contexts/api')>()),
  listContexts: vi.fn(),
}));
vi.mock('./components/EntityForm', () => ({ EntityForm: () => null }));
vi.mock('./components/EntityToolbar', () => ({ EntityToolbar: () => null }));
vi.mock('../../components/PageHeader', () => ({
  PageHeader: ({ actions }: { actions: React.ReactNode }) => (
    <div>{actions}</div>
  ),
}));

const blueprint = {
  blueprint: {
    code: 'product',
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: 'Product',
    status: 'published',
    version: 1,
    views: {},
  },
  attributes: [],
  table_path_attributes: [],
};

describe('EditEntityPage', () => {
  it('shows the localized Sample badge for a sample entity', async () => {
    vi.mocked(getEntityForm).mockResolvedValue({
      blueprint,
      context: {},
      entity: {
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: 1,
        id: '123e4567-e89b-12d3-a456-426614174001',
        is_sample: true,
      },
      reusable_attributes: [],
      reusable_values: [],
      values: [],
    } as unknown as Awaited<ReturnType<typeof getEntityForm>>);
    vi.mocked(getCurrentBlueprint).mockResolvedValue(blueprint as never);
    vi.mocked(listContexts).mockResolvedValue([]);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });

    render(
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <EditEntityPage entityId="123e4567-e89b-12d3-a456-426614174001" />
        </ToastProvider>
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Sample')).toBeTruthy();
  });
});
