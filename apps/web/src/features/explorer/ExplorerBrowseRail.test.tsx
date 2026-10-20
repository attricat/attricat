// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getRelationshipTreeFacetChildren } from '../records/api';
import type { BrowseByField } from './browseBy';
import { ExplorerBrowseRail } from './ExplorerBrowseRail';

vi.mock('../records/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../records/api')>();
  return { ...actual, getRelationshipTreeFacetChildren: vi.fn() };
});

const contextId = '123e4567-e89b-12d3-a456-426614174000';
const shoesId = '123e4567-e89b-12d3-a456-426614174001';
const apparelId = '123e4567-e89b-12d3-a456-426614174002';
const sneakersId = '123e4567-e89b-12d3-a456-426614174003';

const categories: BrowseByField = {
  attribute: {
    code: 'categories',
    name: 'Categories',
    target_blueprint_code: 'category',
    value_type: 'relationship',
  },
  hierarchyField: 'parent',
};
const brand: BrowseByField = {
  attribute: {
    code: 'brand',
    name: 'Brand',
    target_blueprint_code: 'brand',
    value_type: 'relationship',
  },
};

const renderRail = (
  props: Partial<Parameters<typeof ExplorerBrowseRail>[0]> = {},
) => {
  const handlers = { onFieldChange: vi.fn(), onSelect: vi.fn() };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ExplorerBrowseRail
        blueprint="product"
        blueprintName="Product"
        contextId={contextId}
        defaultField={categories}
        field={categories}
        fields={[categories, brand]}
        query="linen"
        selectedIds={[]}
        version={2}
        {...handlers}
        {...props}
      />
    </QueryClientProvider>,
  );
  return handlers;
};

describe('ExplorerBrowseRail', () => {
  beforeEach(() => {
    vi.mocked(getRelationshipTreeFacetChildren).mockImplementation(
      async (input) => ({
        items:
          input.parent_id === shoesId
            ? [
                {
                  id: sneakersId,
                  display: 'Sneakers',
                  count: 2,
                  has_children: false,
                },
              ]
            : input.parent_id === undefined
              ? [
                  {
                    id: shoesId,
                    display: 'Shoes',
                    count: 5,
                    has_children: true,
                  },
                  {
                    id: apparelId,
                    display: 'Apparel',
                    count: 3,
                    has_children: false,
                  },
                ]
              : [],
        selected_paths: input.selected_target_ids?.includes(sneakersId)
          ? [[shoesId, sneakersId]]
          : [],
        next_cursor: null,
      }),
    );
  });

  it('lists root targets alphabetically and loads children on expand', async () => {
    renderRail();
    const rail = await screen.findByRole('complementary', {
      name: 'Browse by Categories',
    });
    const labels = (await screen.findAllByRole('button', { name: /\d$/ })).map(
      (button) => button.textContent,
    );
    expect(labels).toEqual(['Apparel3', 'Shoes5']);
    expect(rail).toBeTruthy();
    expect(getRelationshipTreeFacetChildren).toHaveBeenCalledWith(
      {
        blueprint: { code: 'product', version: 2 },
        context_id: contextId,
        cursor: null,
        hierarchy_field: 'parent',
        query: 'linen',
        source_relationship_field: 'categories',
      },
      expect.anything(),
    );

    await userEvent.click(screen.getByRole('button', { name: 'Expand Shoes' }));
    expect(await screen.findByText('Sneakers')).toBeTruthy();
    expect(
      screen
        .getByRole('button', { name: 'Collapse Shoes' })
        .getAttribute('aria-expanded'),
    ).toBe('true');
  });

  it('selects one target and clears it again', async () => {
    const { onSelect } = renderRail({ selectedIds: [shoesId] });
    await userEvent.click(await screen.findByText('Apparel'));
    expect(onSelect).toHaveBeenLastCalledWith([apparelId]);
    await userEvent.click(screen.getByText('Shoes'));
    expect(onSelect).toHaveBeenLastCalledWith([]);
    await userEvent.click(screen.getByText('All'));
    expect(onSelect).toHaveBeenLastCalledWith([]);
  });

  it('opens the branches that lead to the selection', async () => {
    renderRail({ selectedIds: [sneakersId] });
    expect(await screen.findByText('Sneakers')).toBeTruthy();
    expect(getRelationshipTreeFacetChildren).toHaveBeenCalledWith(
      expect.objectContaining({ selected_target_ids: [sneakersId] }),
      expect.anything(),
    );
    await userEvent.click(
      screen.getByRole('button', { name: 'Collapse Shoes' }),
    );
    expect(screen.queryByText('Sneakers')).toBeNull();
  });

  it('asks each level for the selected branch past its first page', async () => {
    const mock = vi.mocked(getRelationshipTreeFacetChildren);
    const paged = mock.getMockImplementation()!;
    // Sneakers sits on a later page of Shoes, behind a Load more; the API
    // returns it with the first page when the level is asked for it.
    mock.mockImplementation(async (input, signal) => {
      const page = await paged(input, signal);
      if (input.parent_id !== shoesId) return page;
      const included = input.selected_target_ids?.includes(sneakersId);
      return {
        ...page,
        items: included ? page.items : [],
        next_cursor: input.cursor ? null : apparelId,
      };
    });
    renderRail({ selectedIds: [sneakersId] });
    expect(await screen.findByText('Sneakers')).toBeTruthy();
    expect(mock).toHaveBeenCalledWith(
      expect.objectContaining({
        parent_id: shoesId,
        selected_target_ids: [sneakersId],
      }),
      expect.anything(),
    );
    // The root level is requested once, paths included.
    expect(
      mock.mock.calls.filter(([input]) => input.parent_id === undefined),
    ).toHaveLength(1);
    // The page that holds it later does not list it twice.
    await userEvent.click(screen.getByRole('button', { name: 'Load more' }));
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Load more' })).toBeNull(),
    );
    expect(screen.getAllByText('Sneakers')).toHaveLength(1);
  });

  it('opens collapsed when the selection is stale', async () => {
    const mock = vi.mocked(getRelationshipTreeFacetChildren);
    const paged = mock.getMockImplementation()!;
    mock.mockImplementation(async (input, signal) => {
      if (input.selected_target_ids) throw new Error('Unknown target');
      return paged(input, signal);
    });
    renderRail({ selectedIds: [sneakersId] });
    expect(await screen.findByText('Shoes')).toBeTruthy();
    expect(screen.queryByText('Sneakers')).toBeNull();
    expect(screen.queryByText('Unknown target')).toBeNull();
  });

  it("lists the blueprint's own tree under its name", async () => {
    const parent: BrowseByField = {
      attribute: {
        code: 'parent',
        name: 'Parent',
        target_blueprint_code: 'product',
        value_type: 'relationship',
      },
      hierarchyField: 'parent',
      ownTree: true,
    };
    renderRail({ field: parent, fields: [parent, brand] });
    expect(
      await screen.findByRole('complementary', { name: 'Browse by Product' }),
    ).toBeTruthy();
    expect(getRelationshipTreeFacetChildren).toHaveBeenCalledWith(
      {
        blueprint: { code: 'product', version: 2 },
        context_id: contextId,
        cursor: null,
        hierarchy_field: 'parent',
        query: 'linen',
      },
      expect.anything(),
    );
  });

  it('browses a relationship path as a tree of its last target', async () => {
    const model = {
      cardinality: 'one' as const,
      code: 'model',
      name: 'Model',
      target_blueprint_code: 'model',
      value_type: 'relationship' as const,
    };
    const bodyType = {
      ...model,
      code: 'body_type',
      name: 'Body type',
      target_blueprint_code: 'body_type',
    };
    const path: BrowseByField = {
      attribute: {
        cardinality: 'one',
        code: 'model.body_type',
        name: 'Model → Body type',
        target_blueprint_code: 'body_type',
        value_type: 'relationship',
      },
      hierarchyField: 'parent',
      hops: [model, bodyType],
    };
    const { onFieldChange, onSelect } = renderRail({
      field: path,
      fields: [categories, brand, path],
      selectedIds: [sneakersId],
    });
    expect(
      await screen.findByRole('complementary', {
        name: 'Browse by Model → Body type',
      }),
    ).toBeTruthy();
    // The selected node's branch opens, as for a direct field.
    expect(await screen.findByText('Sneakers')).toBeTruthy();
    expect(getRelationshipTreeFacetChildren).toHaveBeenCalledWith(
      {
        blueprint: { code: 'product', version: 2 },
        context_id: contextId,
        cursor: null,
        hierarchy_field: 'parent',
        query: 'linen',
        selected_target_ids: [sneakersId],
        source_relationship_field: 'model.body_type',
      },
      expect.anything(),
    );
    await userEvent.click(screen.getByText('Apparel'));
    expect(onSelect).toHaveBeenLastCalledWith([apparelId]);

    // The field picker lists paths after the direct fields.
    await userEvent.click(screen.getByRole('combobox', { name: 'Browse by' }));
    const options = screen
      .getAllByRole('option')
      .map((option) => option.textContent);
    expect(options).toEqual(['Categories', 'Brand', 'Model → Body type']);
    await userEvent.click(screen.getByRole('option', { name: 'Categories' }));
    expect(onFieldChange).toHaveBeenLastCalledWith('categories');
  });

  it('hides the rail', async () => {
    const { onFieldChange } = renderRail();
    await userEvent.click(
      screen.getByRole('button', { name: 'Hide browse panel' }),
    );
    expect(onFieldChange).toHaveBeenLastCalledWith(false);
  });

  it('offers to reopen on the default field when hidden', async () => {
    const { onFieldChange } = renderRail({
      defaultField: brand,
      field: undefined,
      fields: [categories, brand],
    });
    await userEvent.click(
      screen.getByRole('button', { name: 'Show browse panel' }),
    );
    expect(onFieldChange).toHaveBeenLastCalledWith('brand');
  });

  it('renders nothing without a field to browse', () => {
    const { container } = render(
      <ExplorerBrowseRail
        blueprint="product"
        blueprintName="Product"
        contextId={contextId}
        defaultField={undefined}
        field={undefined}
        fields={[]}
        query={undefined}
        selectedIds={[]}
        version={undefined}
        onFieldChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    expect(container.childElementCount).toBe(0);
  });
});
