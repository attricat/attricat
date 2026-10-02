// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
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
  updateEntity,
} from './api';
import { entityQueryKeys } from './queryKeys';
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
    (
      {
        contextPicker,
        expectedUpdatedAt,
        onSubmit,
      }: {
        contextPicker: React.ReactNode;
        expectedUpdatedAt: string;
        onSubmit: (input: unknown) => void;
      },
      ref,
    ) => {
      const handle = { applySmartFillValues, clearDraft: vi.fn() };
      if (typeof ref === 'function') ref(handle);
      else if (ref) ref.current = handle;
      return (
        <div>
          {contextPicker}
          <button
            onClick={() =>
              onSubmit({
                expected_updated_at: expectedUpdatedAt,
                values: [],
                relationships: [],
                remove_values: [],
              })
            }
          >
            Save {expectedUpdatedAt}
          </button>
        </div>
      );
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
  const renderPage = (
    queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    }),
  ) => {
    return render(
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <EditEntityPage entityId="123e4567-e89b-12d3-a456-426614174001" />
        </ToastProvider>
      </QueryClientProvider>,
    );
  };

  it('waits for an opening refresh and refreshes its cache after saving', async () => {
    const entityId = '123e4567-e89b-12d3-a456-426614174001';
    const response = (updated_at: string) =>
      ({
        blueprint,
        entity: {
          id: entityId,
          blueprint_id: blueprint.blueprint.id,
          blueprint_version: 1,
          updated_at,
          is_sample: false,
        },
        context: {},
        reusable_attributes: [],
        reusable_values: [],
        values: [],
      }) as unknown as Awaited<ReturnType<typeof getEntityForm>>;
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    client.setQueryData(entityQueryKeys.form(entityId), response('old'));
    let finish!: (value: Awaited<ReturnType<typeof getEntityForm>>) => void;
    vi.mocked(getEntityForm).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    vi.mocked(getCurrentBlueprint).mockResolvedValue(blueprint as never);
    vi.mocked(listContexts).mockResolvedValue([]);
    const view = renderPage(client);
    expect(screen.queryByRole('button', { name: 'Save old' })).toBeNull();
    await act(async () => {
      finish(response('fresh'));
    });
    await screen.findByRole('button', { name: 'Save fresh' });
    vi.mocked(updateEntity).mockResolvedValue({ id: entityId } as never);
    vi.mocked(getEntityForm).mockResolvedValue(response('saved'));
    fireEvent.click(screen.getByRole('button', { name: 'Save fresh' }));
    await waitFor(() =>
      expect(client.getQueryData(entityQueryKeys.form(entityId))).toEqual(
        response('saved'),
      ),
    );
    view.unmount();
    renderPage(client);
    fireEvent.click(await screen.findByRole('button', { name: 'Save saved' }));
    await waitFor(() =>
      expect(updateEntity).toHaveBeenLastCalledWith(
        entityId,
        expect.objectContaining({ expected_updated_at: 'saved' }),
      ),
    );
    expect(screen.queryByRole('button', { name: 'Save old' })).toBeNull();
  });

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
