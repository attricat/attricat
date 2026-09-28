// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { forwardRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import {
  getCurrentBlueprint,
  getEntityForm,
  getResolvedEntityPreview,
} from './api';
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
const applySmartFillValues = vi.fn();
const drawerSpy = vi.fn();
vi.mock('./components/EntityAgentDrawer', () => ({
  EntityAgentDrawer: (props: {
    contextId?: string;
    open: boolean;
    draft: { onApply: (fields: Record<string, string>) => void };
  }) => {
    drawerSpy(props);
    return props.open ? (
      <button onClick={() => props.draft.onApply({ title: 'Suggested' })}>
        Apply proposal
      </button>
    ) : null;
  },
}));
vi.mock('./components/EntityForm', () => ({
  EntityForm: forwardRef(
    ({ contextPicker }: { contextPicker: React.ReactNode }, ref) => {
      if (typeof ref === 'function') ref({ applySmartFillValues });
      else if (ref) ref.current = { applySmartFillValues };
      return <div>{contextPicker}</div>;
    },
  ),
}));
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
  const renderPage = () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    return render(
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <EditEntityPage entityId="123e4567-e89b-12d3-a456-426614174001" />
        </ToastProvider>
      </QueryClientProvider>,
    );
  };

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
    renderPage();

    expect(await screen.findByText('Sample')).toBeTruthy();
  });

  it('opens Smart Fill for the active entity context', async () => {
    vi.mocked(getEntityForm).mockResolvedValue({
      blueprint,
      context: {},
      entity: {
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: 1,
        id: '123e4567-e89b-12d3-a456-426614174001',
        is_sample: false,
      },
      reusable_attributes: [],
      reusable_values: [],
      values: [],
    } as unknown as Awaited<ReturnType<typeof getEntityForm>>);
    vi.mocked(getCurrentBlueprint).mockResolvedValue(blueprint as never);
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '123e4567-e89b-12d3-a456-426614174002',
        code: 'default',
        data: {},
      },
    ] as Awaited<ReturnType<typeof listContexts>>);
    vi.mocked(getResolvedEntityPreview).mockResolvedValue({
      values: {},
    } as never);
    const user = userEvent.setup();

    renderPage();
    await user.click(await screen.findByRole('button', { name: 'Smart fill' }));

    expect(screen.getByRole('button', { name: 'Apply proposal' })).toBeTruthy();
    expect(drawerSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        contextId: '123e4567-e89b-12d3-a456-426614174002',
        open: true,
      }),
    );
  });

  it('binds the editing sidebar to the selected context', async () => {
    applySmartFillValues.mockClear();
    const defaultId = '123e4567-e89b-12d3-a456-426614174002';
    const otherId = '123e4567-e89b-12d3-a456-426614174003';
    vi.mocked(getEntityForm).mockResolvedValue({
      blueprint,
      entity: {
        id: '123e4567-e89b-12d3-a456-426614174001',
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: 1,
        is_sample: false,
      },
      values: [],
      reusable_values: [],
      reusable_attributes: [],
      context: {},
    } as never);
    vi.mocked(getCurrentBlueprint).mockResolvedValue(blueprint as never);
    vi.mocked(getResolvedEntityPreview).mockResolvedValue({
      values: {},
    } as never);
    vi.mocked(listContexts).mockResolvedValue([
      { id: defaultId, code: 'default', data: {} },
      { id: otherId, code: 'other', data: {} },
    ] as never);
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: 'Smart fill' }));
    expect(drawerSpy).toHaveBeenCalledWith(
      expect.objectContaining({ contextId: defaultId, open: true }),
    );
    fireEvent.click(screen.getByRole('tab', { name: 'other', hidden: true }));
    expect(drawerSpy).toHaveBeenCalledWith(
      expect.objectContaining({ contextId: otherId, open: true }),
    );
    await user.click(screen.getByRole('button', { name: 'Apply proposal' }));
    expect(applySmartFillValues).toHaveBeenCalledWith({ title: 'Suggested' });
  });
});
