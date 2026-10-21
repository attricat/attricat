// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  defaultParseSearch,
  RouterProvider,
} from '@tanstack/react-router';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { listBlueprints } from '../../blueprints/api';
import { parseExplorerRouteSearch } from '../../explorer/search';
import { IncomingRelationshipListDisplay } from './IncomingRelationshipListDisplay';

vi.mock('../../blueprints/api', () => ({ listBlueprints: vi.fn() }));

const recordId = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d';

const renderDisplay = (
  relationships: { source_blueprint: string; field: string }[],
) => {
  const rootRoute = createRootRoute();
  const recordRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/records/$recordId',
    component: () => (
      <IncomingRelationshipListDisplay
        node={{
          type: 'incoming_relationship_list',
          label: 'Linked records',
          page_size: 10,
          relationships,
        }}
        recordId={recordId}
      />
    ),
  });
  const router = createRouter({
    history: createMemoryHistory({ initialEntries: [`/records/${recordId}`] }),
    routeTree: rootRoute.addChildren([recordRoute]),
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
};

const explorerSearch = (link: HTMLElement) => {
  const url = new URL(link.getAttribute('href') ?? '', 'http://localhost');
  expect(url.pathname).toBe('/');
  return parseExplorerRouteSearch(defaultParseSearch(url.search));
};

describe('IncomingRelationshipListDisplay', () => {
  it('links to the Explorer filtered to records linking here, in a new tab', async () => {
    renderDisplay([{ source_blueprint: 'model', field: 'brand' }]);

    const link = await screen.findByRole('link', { name: /Linked records/ });
    expect(link.getAttribute('target')).toBe('_blank');
    expect(
      screen.getByRole('img', { name: 'Opens in a new tab' }),
    ).toBeTruthy();
    expect(explorerSearch(link)).toEqual({
      blueprint: 'model',
      allVersions: true,
      relationshipFacets: [{ field: 'brand', selectedIds: [recordId] }],
    });
    expect(listBlueprints).not.toHaveBeenCalled();
  });

  it('links to each relationship separately, named by its blueprint', async () => {
    vi.mocked(listBlueprints).mockResolvedValue([
      { code: 'model', name: 'Model' },
      { code: 'dealer', name: 'Dealer' },
    ] as Awaited<ReturnType<typeof listBlueprints>>);
    renderDisplay([
      { source_blueprint: 'model', field: 'brand' },
      { source_blueprint: 'dealer', field: 'brands' },
    ]);

    const dealers = await screen.findByRole('link', {
      name: /Linked records · Dealer/,
    });
    expect(explorerSearch(dealers).relationshipFacets).toEqual([
      { field: 'brands', selectedIds: [recordId] },
    ]);
    expect(
      screen.getByRole('link', { name: /Linked records · Model/ }),
    ).toBeTruthy();
  });
});
