// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  getBlueprintByCode,
  searchEntities,
  type BlueprintWithAttributes,
  type EntitySearchResponse,
} from '../entities/api';
import { relationshipPickerMessageType } from '../entities/components/useRecentlyPreviewedEntities';
import type { AttributeFilterRequest } from './attributeFilterValues';
import { maximumAttributeFilters } from './constants';
import { ExplorerFilterBar } from './ExplorerFilterBar';
import type {
  ExplorerRelationshipFacet,
  RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import type { AttributeFilter } from './search';

vi.mock('../entities/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../entities/api')>();
  return { ...actual, getBlueprintByCode: vi.fn(), searchEntities: vi.fn() };
});

vi.mock('../entities/components/useRelationshipSelectionLabels', () => ({
  useRelationshipSelectionLabels: () => new Map([[targetId, 'Acme']]),
}));

const brandBlueprintId = '123e4567-e89b-12d3-a456-426614174001';
const targetId = '123e4567-e89b-12d3-a456-426614174002';

const brandBlueprint: BlueprintWithAttributes = {
  attributes: [],
  blueprint: {
    code: 'brand',
    id: brandBlueprintId,
    name: 'Brand',
    status: 'published',
    version: 1,
    views: {},
  },
  table_path_attributes: [],
};

const targetPage: EntitySearchResponse = {
  blueprint: brandBlueprint,
  hidden_outdated_count: null,
  hidden_outdated_count_capped: false,
  items: [
    {
      blueprint_version: 1,
      display: { default: 'Acme' },
      id: targetId,
      is_sample: false,
      match_explanations: [],
      preview: {},
      schema_outdated: false,
      table_values: {},
    },
  ],
  next_cursor: null,
  result_version_scope: { kind: 'single', version: 1 },
  total_count: null,
  total_count_capped: false,
};

/** Holds filters and facets in state the way the Explorer URL would. */
const StatefulBar = ({
  initialFacets = [],
  initialFilters = [],
}: {
  initialFacets?: ExplorerRelationshipFacet[];
  initialFilters?: AttributeFilter[];
}) => {
  const [filters, setFilters] = useState(initialFilters);
  const [facets, setFacets] = useState(initialFacets);
  return (
    <ExplorerFilterBar
      attributes={[{ code: 'stock', value_type: 'integer' }]}
      blueprint="product"
      blueprints={[brandBlueprint.blueprint]}
      facets={facets}
      filters={filters}
      onAdd={vi.fn()}
      onRemove={(index) =>
        setFilters((current) =>
          current.filter((_, position) => position !== index),
        )
      }
      onUpdate={vi.fn()}
      onUpdateFacet={(field, updates) =>
        setFacets((current) => [
          ...current.filter((facet) => facet.sourceRelationship.code !== field),
          ...(updates.selectedIds?.length
            ? [
                {
                  selectedIds: updates.selectedIds,
                  sourceRelationship: relationship(field, 'brand'),
                },
              ]
            : []),
        ])
      }
      relationshipAttributes={[relationship('brand', 'brand')]}
    />
  );
};

const relationship = (
  code: string,
  target: string,
): RelationshipFilterAttribute => ({
  cardinality: 'many',
  code,
  target_blueprint_code: target,
  value_type: 'relationship',
});

const blueprint = (
  code: string,
  attributes: RelationshipFilterAttribute[],
): BlueprintWithAttributes => ({
  attributes,
  blueprint: {
    code,
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: code,
    status: 'published',
    version: 1,
    views: {},
  },
  table_path_attributes: [],
});

describe('ExplorerFilterBar', () => {
  it('discovers relationship paths up to three hops and returns the terminal target', async () => {
    vi.mocked(searchEntities).mockResolvedValue(targetPage);
    vi.mocked(getBlueprintByCode).mockImplementation((code) => {
      if (code === 'family')
        return Promise.resolve(
          blueprint(code, [relationship('class', 'class')]),
        );
      if (code === 'class')
        return Promise.resolve(blueprint(code, [relationship('kind', 'kind')]));
      return Promise.resolve(blueprint(code, []));
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExplorerFilterBar
          attributes={[]}
          blueprint="product"
          filters={[]}
          onAdd={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
          relationshipAttributes={[relationship('family', 'family')]}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    const field = screen.getByRole('combobox', { name: 'Field' });
    await user.click(field);
    const thirdHop = await screen.findByRole('option', {
      name: /family\.class\.kind/i,
    });
    await user.click(thirdHop);

    expect(
      await screen.findByRole('dialog', {
        name: /^Select family\.class\.kind \(kind\)/,
      }),
    ).toBeTruthy();
  });

  it('keeps local attribute names and status options when a table column has the same code', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={queryClient}>
        <ExplorerFilterBar
          attributes={[
            {
              code: 'state',
              name: 'Workflow state',
              value_type: 'string',
              value_schema: {
                type: 'string',
                enum: ['draft', 'live'],
                'x-attricat-status': {
                  version: 1,
                  options: [
                    { code: 'draft', label: 'Draft' },
                    { code: 'live', label: 'Live' },
                  ],
                },
              },
            },
          ]}
          blueprint="product"
          filters={[]}
          onAdd={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
          pathAttributes={[{ code: 'state', value_type: 'string' }]}
        />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    await user.click(screen.getByRole('combobox', { name: 'Field' }));
    await user.click(
      await screen.findByRole('option', { name: /Workflow state/ }),
    );
    await user.click(screen.getByRole('combobox', { name: 'Value' }));
    expect(await screen.findByRole('option', { name: 'Live' })).toBeTruthy();
  });

  it('keeps keyboard focus in the list when filters are removed', async () => {
    const Picker = () => {
      const [filters, setFilters] = useState<AttributeFilter[]>([
        { field: 'stock', operator: 'gt', value: 5 },
        { field: 'stock', operator: 'lt', value: 50 },
      ]);
      return (
        <ExplorerFilterBar
          attributes={[{ code: 'stock', value_type: 'integer' }]}
          blueprint="product"
          filters={filters}
          onAdd={vi.fn()}
          onRemove={(index) =>
            setFilters((current) =>
              current.filter((_, position) => position !== index),
            )
          }
          onUpdate={vi.fn()}
        />
      );
    };
    render(
      <QueryClientProvider client={new QueryClient()}>
        <Picker />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    const chips = () =>
      screen
        .getAllByRole('button')
        .filter((element) => element.classList.contains('MuiChip-root'));

    chips()[0].focus();
    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(1);
    expect(document.activeElement).toBe(chips()[0]);

    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(0);
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Add filter' }),
    );
  });

  it('opens a relationship picker immediately and shows the selection as a chip', async () => {
    vi.mocked(getBlueprintByCode).mockResolvedValue(brandBlueprint);
    vi.mocked(searchEntities).mockResolvedValue(targetPage);
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <StatefulBar />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    await user.click(screen.getByRole('combobox', { name: 'Field' }));
    await user.click(await screen.findByRole('option', { name: /brand/i }));

    const dialog = await screen.findByRole('dialog', { name: /^Select brand/ });
    const entityLink = await screen.findByRole('link', { name: 'Acme' });
    expect(entityLink.getAttribute('target')).toBe('_blank');
    const href = entityLink.getAttribute('href');
    if (!href) throw new Error('Entity link is missing its href');
    const pickerToken = new URL(href, location.href).searchParams.get(
      'relationshipPicker',
    );
    expect(pickerToken).toBeTruthy();
    window.dispatchEvent(
      new MessageEvent('message', {
        data: {
          entityId: targetId,
          token: pickerToken,
          type: relationshipPickerMessageType,
        },
        origin: window.location.origin,
      }),
    );

    await waitFor(() => expect(dialog.textContent).toContain('1 selected'));
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(
      await screen.findByRole('button', { name: /^brand: Acme/ }),
    ).toBeTruthy();
  });

  it('moves focus across attribute and relationship chips as they are removed', async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <StatefulBar
          initialFacets={[
            {
              selectedIds: [targetId],
              sourceRelationship: relationship('brand', 'brand'),
            },
          ]}
          initialFilters={[{ field: 'stock', operator: 'gt', value: 5 }]}
        />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    const chips = () =>
      screen
        .queryAllByRole('button')
        .filter((element) => element.classList.contains('MuiChip-root'));

    chips()[0].focus();
    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(1);
    expect(document.activeElement).toBe(chips()[0]);
    expect(chips()[0].textContent).toBe('brand: Acme');

    await user.keyboard('{Delete}');
    expect(chips()).toHaveLength(0);
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Add filter' }),
    );
  });

  it('edits a filter from its chip', async () => {
    const onUpdate = vi.fn();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ExplorerFilterBar
          attributes={[{ code: 'stock', value_type: 'integer' }]}
          blueprint="product"
          filters={[{ field: 'stock', operator: 'gt', value: 5 }]}
          onAdd={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={onUpdate}
        />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: /^stock > "5"/ }));
    const dialog = await screen.findByRole('dialog', { name: /^Edit filter/ });
    const value = screen.getByRole('spinbutton', { name: 'Value' });
    expect((value as HTMLInputElement).value).toBe('5');
    await user.clear(value);
    await user.type(value, '10{Enter}');

    await waitFor(() => expect(dialog.isConnected).toBe(false));
    expect(onUpdate).toHaveBeenCalledWith(0, {
      field: 'stock',
      operator: 'gt',
      value: 10,
    });
  });

  it('opens each new cell filter request once with its draft', async () => {
    const queryClient = new QueryClient();
    const bar = (filterRequest?: AttributeFilterRequest) => (
      <QueryClientProvider client={queryClient}>
        <ExplorerFilterBar
          attributes={[{ code: 'stock', value_type: 'integer' }]}
          blueprint="product"
          filterRequest={filterRequest}
          filters={[]}
          onAdd={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
        />
      </QueryClientProvider>
    );
    const request: AttributeFilterRequest = {
      id: 1,
      draft: { field: 'stock', operator: 'eq', value: '7' },
    };
    const { rerender } = render(bar());
    expect(screen.queryByRole('dialog')).toBeNull();

    rerender(bar(request));
    await screen.findByRole('dialog', { name: /^Add filter/ });
    expect(
      (screen.getByRole('spinbutton', { name: 'Value' }) as HTMLInputElement)
        .value,
    ).toBe('7');

    const user = userEvent.setup();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    rerender(bar(request));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('says when the filter limit is reached', () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ExplorerFilterBar
          attributes={[{ code: 'stock', value_type: 'integer' }]}
          blueprint="product"
          filters={Array.from({ length: maximumAttributeFilters }, (_, i) => ({
            field: 'stock',
            operator: 'gt' as const,
            value: i,
          }))}
          onAdd={vi.fn()}
          onRemove={vi.fn()}
          onUpdate={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(
      screen.getByText(
        new RegExp(`${maximumAttributeFilters} attribute filters`),
      ),
    ).toBeTruthy();
  });
});
