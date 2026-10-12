// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
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
          ? [[shoesId]]
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

  it('hides the rail', async () => {
    const { onFieldChange } = renderRail();
    await userEvent.click(
      screen.getByRole('button', { name: 'Hide browse panel' }),
    );
    expect(onFieldChange).toHaveBeenLastCalledWith(false);
  });

  it('offers to reopen when hidden', async () => {
    const { onFieldChange } = renderRail({
      field: undefined,
      fields: [brand, categories],
    });
    await userEvent.click(
      screen.getByRole('button', { name: 'Show browse panel' }),
    );
    expect(onFieldChange).toHaveBeenLastCalledWith('categories');
  });

  it('renders nothing without a field to browse', () => {
    const { container } = render(
      <ExplorerBrowseRail
        blueprint="product"
        blueprintName="Product"
        contextId={contextId}
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
