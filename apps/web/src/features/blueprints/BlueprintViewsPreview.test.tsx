// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from '../contexts/api';
import {
  getBlueprintByCode,
  getRecordLabels,
  getResolvedRecordPreview,
  searchRecords,
  type RecordSearchResponse,
} from '../records/api';
import {
  DefinitionCursorContext,
  DefinitionRevealContext,
} from '../definition-editor/definitionReveal';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';

vi.mock('../records/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../records/api')>()),
  getBlueprintByCode: vi.fn(),
  getRecordLabels: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
  searchRecords: vi.fn(),
}));
vi.mock('../contexts/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../contexts/api')>()),
  listContexts: vi.fn(),
}));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const golfId = '123e4567-e89b-12d3-a456-426614174001';
const dealerId = '123e4567-e89b-12d3-a456-426614174002';
const defaultContextId = '123e4567-e89b-12d3-a456-426614174009';
const carBlueprint = {
  blueprint: {
    code: 'car',
    id: blueprintId,
    name: 'Car',
    status: 'published' as const,
    version: 1,
    views: {},
  },
  attributes: [],
  table_path_attributes: [],
};
const carPage: RecordSearchResponse = {
  blueprint: carBlueprint,
  items: [
    {
      id: golfId,
      blueprint_version: 1,
      schema_outdated: false,
      system_tags: [],
      is_sample: false,
      display: { default: 'Golf' },
      preview: {},
      table_values: {},
    },
  ],
  next_cursor: null,
  total_count: null,
  total_count_capped: false,
  result_version_scope: { kind: 'single', version: 1 },
  hidden_outdated_count: null,
  hidden_outdated_count_capped: false,
};

describe('BlueprintViewsPreview', () => {
  it('leads from a field to the fields connected to it', async () => {
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <BlueprintViewsPreview
          attributes={[
            { code: 'vin', name: 'VIN', value_type: 'string' },
            { code: 'make', name: 'Make', value_type: 'string' },
          ]}
          definition={`
unique_keys:
  - code: vin_make
    attributes: [vin, make]
`}
          views={{
            detail: {
              type: 'stack',
              children: [
                { type: 'field', field: 'vin' },
                { type: 'field', field: 'make' },
              ],
            },
          }}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Details of VIN' }));
    await user.click(screen.getByRole('button', { name: 'Make' }));

    await waitFor(() =>
      expect(document.activeElement).toBe(
        screen.getByRole('textbox', { name: 'Make' }),
      ),
    );
  });

  it('leads from a field and a view to where the definition declares them', async () => {
    const user = userEvent.setup();
    const reveal = vi.fn();
    const definition = `attributes:
  - code: vin
    value_type: string
views:
  detail:
    type: stack
    children:
      - {type: field, field: vin}
`;
    render(
      <QueryClientProvider client={new QueryClient()}>
        <DefinitionRevealContext value={reveal}>
          <BlueprintViewsPreview
            attributes={[
              { code: 'vin', name: 'VIN', value_type: 'string' },
              { code: 'make', name: 'Make', value_type: 'string' },
            ]}
            definition={definition}
            views={{
              detail: {
                type: 'stack',
                children: [
                  { type: 'field', field: 'vin' },
                  { type: 'field', field: 'make' },
                ],
              },
            }}
          />
        </DefinitionRevealContext>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Details of VIN' }));
    await user.click(
      screen.getByRole('button', { name: 'Show in definition' }),
    );
    const code = definition.indexOf('vin');
    expect(reveal).toHaveBeenLastCalledWith({
      block: expect.objectContaining({
        start: definition.indexOf('code: vin'),
      }),
      focus: { end: code + 'vin'.length, start: code },
    });

    // A field the edited definition no longer declares has nowhere to lead.
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: 'Details of Make' }));
    expect(
      screen.getByRole('button', { name: 'Show in definition' }),
    ).toHaveProperty('disabled', true);
    await user.keyboard('{Escape}');

    await user.click(
      screen.getByRole('button', {
        name: 'Show the detail view in the definition',
      }),
    );
    expect(reveal).toHaveBeenLastCalledWith(
      expect.objectContaining({
        focus: {
          end: definition.indexOf('detail:') + 'detail'.length,
          start: definition.indexOf('detail:'),
        },
      }),
    );
  });

  it('follows the definition cursor between views and attributes', () => {
    const definition = `attributes:
  - code: vin
    value_type: string
views:
  table:
    type: table
    columns:
      - {field: vin}
`;
    const views = {
      table: { type: 'table' as const, columns: [{ field: 'vin' }] },
    };
    const preview = (cursor: number) => (
      <QueryClientProvider client={new QueryClient()}>
        <DefinitionCursorContext value={cursor}>
          <BlueprintViewsPreview
            attributes={[{ code: 'vin', name: 'VIN', value_type: 'string' }]}
            definition={definition}
            views={views}
          />
        </DefinitionCursorContext>
      </QueryClientProvider>
    );
    const { rerender } = render(preview(definition.indexOf('type: table')));
    expect(screen.getByRole('tab', { name: 'table' })).toHaveProperty(
      'ariaSelected',
      'true',
    );

    rerender(preview(definition.indexOf('value_type')));
    expect(screen.getByRole('tab', { name: 'Edit' })).toHaveProperty(
      'ariaSelected',
      'true',
    );
  });

  it('previews the views with an existing record and its links', async () => {
    // The picker scrolls to the top when records arrive; jsdom cannot scroll.
    Element.prototype.scrollTo = vi.fn();
    vi.mocked(getBlueprintByCode).mockResolvedValue(carBlueprint);
    vi.mocked(searchRecords).mockResolvedValue(carPage);
    vi.mocked(getRecordLabels).mockResolvedValue({
      items: [
        { id: golfId, blueprint_code: 'car', display: { default: 'Golf' } },
      ],
    });
    vi.mocked(listContexts).mockResolvedValue([
      { id: defaultContextId, code: 'default', data: {}, parent_id: null },
    ]);
    vi.mocked(getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: golfId,
        blueprint_id: blueprintId,
        blueprint_version: 1,
        is_sample: false,
      },
      requested_context: {
        id: defaultContextId,
        code: 'default',
        data: {},
        parent_id: null,
      },
      values: {
        model: {
          value: 'Golf',
          source_context: { id: defaultContextId, code: 'default' },
        },
        dealer: {
          value: { items: [{ id: dealerId, display: 'Main dealer' }] },
          source_context: { id: defaultContextId, code: 'default' },
        },
        // Stored as text before this draft made `doors` an integer.
        doors: {
          value: 'five',
          source_context: { id: defaultContextId, code: 'default' },
        },
      },
      reusable_attributes: [],
      reusable_values: {},
    });
    const user = userEvent.setup();
    // Linked records render as links to their record pages.
    const rootRoute = createRootRoute();
    const router = createRouter({
      history: createMemoryHistory({ initialEntries: ['/'] }),
      routeTree: rootRoute.addChildren([
        createRoute({
          getParentRoute: () => rootRoute,
          path: '/',
          component: () => (
            <BlueprintViewsPreview
              attributes={[
                { code: 'model', name: 'Model', value_type: 'string' },
                { code: 'doors', name: 'Doors', value_type: 'integer' },
                {
                  code: 'dealer',
                  name: 'Dealer',
                  value_type: 'relationship',
                  cardinality: 'many',
                  target_blueprint_code: 'dealer',
                },
              ]}
              blueprintCode="car"
              views={{
                detail: {
                  type: 'stack',
                  children: [
                    { type: 'field', field: 'model' },
                    { type: 'field', field: 'doors' },
                    { type: 'field', field: 'dealer' },
                  ],
                },
              }}
            />
          ),
        }),
        createRoute({
          getParentRoute: () => rootRoute,
          path: '/records/$recordId',
        }),
      ]),
    });
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );

    const choose = await screen.findByRole('button', {
      name: 'Preview with a record…',
    });
    await waitFor(() =>
      expect((choose as HTMLButtonElement).disabled).toBe(false),
    );
    await user.click(choose);
    const dialog = await screen.findByRole('dialog');
    await user.click(await within(dialog).findByText('Golf'));
    const apply = within(dialog).queryByRole('button', { name: 'Apply' });
    if (apply) await user.click(apply);

    // The sandbox starts from the record, and its links show as they are.
    await waitFor(() =>
      expect(
        (screen.getByRole('textbox', { name: 'Model' }) as HTMLInputElement)
          .value,
      ).toBe('Golf'),
    );
    expect(screen.getByText('Showing Golf')).toBeTruthy();
    expect(screen.getByText('Main dealer')).toBeTruthy();
    expect(getResolvedRecordPreview).toHaveBeenCalledWith(
      golfId,
      defaultContextId,
    );
    // A value that no longer fits its attribute is left out and named.
    expect(
      screen.getByText(/changed type in this version.*Doors/),
    ).toBeTruthy();

    // Clearing the record returns to an empty sandbox.
    await user.click(
      within(
        screen.getByText('Showing Golf').closest('.MuiChip-root')!,
      ).getByTestId('CancelIcon'),
    );
    expect(
      (screen.getByRole('textbox', { name: 'Model' }) as HTMLInputElement)
        .value,
    ).toBe('');
    expect(screen.queryByText('Main dealer')).toBeNull();
  });
});
