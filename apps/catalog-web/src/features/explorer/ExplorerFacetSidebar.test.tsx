// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  getBlueprintByCode,
  searchEntities,
  type EntitySearchResponse,
} from '../entities/api';
import type { AttributeContext } from '../contexts/api';
import { ExplorerFacetSidebar } from './ExplorerFacetSidebar';
import { relationshipPickerMessageType } from '../entities/components/useRecentlyPreviewedEntities';
import type {
  ExplorerRelationshipFacet,
  RelationshipFilterAttribute,
} from './relationshipFilterTypes';

vi.mock('../entities/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../entities/api')>();
  return { ...actual, getBlueprintByCode: vi.fn(), searchEntities: vi.fn() };
});

vi.mock('../entities/components/useRelationshipSelectionLabels', () => ({
  useRelationshipSelectionLabels: () => new Map([[targetId, 'Acme']]),
}));

const brandBlueprintId = '123e4567-e89b-12d3-a456-426614174000';
const productBlueprintId = '123e4567-e89b-12d3-a456-426614174001';
const targetId = '123e4567-e89b-12d3-a456-426614174002';
const relationship: RelationshipFilterAttribute = {
  cardinality: 'many',
  code: 'brand',
  target_blueprint_code: 'brand',
  value_type: 'relationship',
};

const targetPage: EntitySearchResponse = {
  blueprint: {
    blueprint: {
      code: 'brand',
      id: brandBlueprintId,
      name: 'Brand',
      status: 'published',
      version: 1,
      views: {},
    },
    attributes: [],
    table_path_attributes: [],
  },
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

const defaultContext = {
  code: 'default',
  data: {},
  id: targetId,
  parent_id: null,
};

const Harness = ({
  contexts = [defaultContext],
}: {
  contexts?: AttributeContext[];
}) => {
  const [facets, setFacets] = useState<ExplorerRelationshipFacet[]>([]);
  return (
    <ExplorerFacetSidebar
      attributeFilters={[]}
      attributes={[relationship]}
      blueprint="product"
      blueprints={[
        {
          code: 'product',
          id: productBlueprintId,
          name: 'Product',
          status: 'published',
          version: 1,
          views: {},
        },
        {
          code: 'brand',
          id: brandBlueprintId,
          name: 'Brand',
          status: 'published',
          version: 1,
          views: {},
        },
      ]}
      contextCode="default"
      contexts={contexts}
      facets={facets}
      onAddAttributeFilter={vi.fn()}
      onContextChange={vi.fn()}
      onRemoveAttributeFilter={vi.fn()}
      onUpdate={(field, updates) =>
        setFacets(
          updates.selectedIds?.length
            ? [
                {
                  selectedIds: updates.selectedIds,
                  sourceRelationship: { ...relationship, code: field },
                },
              ]
            : [],
        )
      }
      onUpdateAttributeFilter={vi.fn()}
      relationshipAttributes={[relationship]}
    />
  );
};

const renderSidebar = (contexts?: AttributeContext[]) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <Harness contexts={contexts} />
    </QueryClientProvider>,
  );
};

describe('ExplorerFacetSidebar', () => {
  it('offers a context choice only beside the default context', () => {
    renderSidebar();
    expect(screen.queryByRole('combobox', { name: 'Context' })).toBeNull();
    cleanup();
    renderSidebar([
      defaultContext,
      { ...defaultContext, code: 'storefront', id: brandBlueprintId },
    ]);
    expect(screen.getByRole('combobox', { name: 'Context' })).toBeTruthy();
  });

  it('opens a relationship picker immediately and applies selections live', async () => {
    vi.mocked(getBlueprintByCode).mockResolvedValue({
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
    });
    vi.mocked(searchEntities).mockResolvedValue(targetPage);
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    });
    const user = userEvent.setup();
    renderSidebar();

    expect(screen.queryByRole('heading', { name: 'Brand' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Add filter' }));
    await user.click(screen.getByRole('combobox', { name: 'Field' }));
    await user.click(await screen.findByRole('option', { name: /brand/i }));

    const dialog = await screen.findByRole('dialog', { name: /^Select brand/ });
    expect(screen.queryByRole('heading', { name: 'Brand' })).toBeNull();

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
    expect(dialog.textContent).toContain('Acme');
    await user.click(screen.getByRole('button', { name: 'Remove Acme' }));
    await user.click(screen.getByRole('button', { name: 'Select Acme' }));
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByRole('heading', { name: /brand/i })).toBeTruthy();
  });
});
