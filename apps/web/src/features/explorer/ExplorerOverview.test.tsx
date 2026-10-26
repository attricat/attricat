// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ApiRequestError, type Blueprint } from '../records/api';
import { invalidInputErrorCode } from '../../api/errors';
import { ExplorerOverview } from './ExplorerOverview';
import { useExplorerOverview } from './useExplorerOverview';

vi.mock('./useExplorerOverview', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./useExplorerOverview')>();
  return { ...actual, useExplorerOverview: vi.fn() };
});
vi.mock('./ExplorerOverviewSection', () => ({
  ExplorerOverviewSection: ({ blueprint }: { blueprint: Blueprint }) => (
    <section aria-label={blueprint.name} />
  ),
}));

type OverviewResult = ReturnType<typeof useExplorerOverview>[number];

const blueprint = (code: string): Blueprint => ({
  code,
  id: crypto.randomUUID(),
  name: code,
  status: 'published',
  version: 1,
  views: {},
});
const blueprints = ['products', 'brands', 'colors'].map(blueprint);

const withItems = (count: number) =>
  ({
    data: { items: Array.from({ length: count }, () => ({})) },
    error: null,
    isError: false,
    isPending: false,
  }) as unknown as OverviewResult;
const failed = (error: Error) =>
  ({
    data: undefined,
    error,
    isError: true,
    isPending: false,
  }) as unknown as OverviewResult;
const pending = {
  data: undefined,
  error: null,
  isError: false,
  isPending: true,
} as unknown as OverviewResult;
const rejection = (message: string) =>
  new ApiRequestError(400, message, invalidInputErrorCode);

const renderOverview = (results: OverviewResult[]) => {
  vi.mocked(useExplorerOverview).mockReturnValue(results);
  render(
    <ExplorerOverview
      blueprints={blueprints}
      contextCode="default"
      contextCodes={['default']}
      contextId={undefined}
      onExplore={vi.fn()}
      publicationSortAvailable
      search={{ query: 'color:red' }}
    />,
  );
};

describe('ExplorerOverview', () => {
  beforeEach(() => vi.mocked(useExplorerOverview).mockReset());

  it('shows only blueprints with matches or unexpected failures', () => {
    renderOverview([
      withItems(2),
      failed(rejection('Unknown field color')),
      failed(new ApiRequestError(500, 'Server error')),
    ]);

    expect(screen.getByRole('region', { name: 'products' })).toBeTruthy();
    expect(screen.queryByRole('region', { name: 'brands' })).toBeNull();
    expect(screen.getByRole('region', { name: 'colors' })).toBeTruthy();
  });

  it('says nothing matched when results are empty or rejected', () => {
    renderOverview([
      withItems(0),
      failed(rejection('Unknown field color')),
      withItems(0),
    ]);

    expect(screen.getByText('No records matched this search.')).toBeTruthy();
    expect(screen.queryByRole('region')).toBeNull();
  });

  it('explains the rejection when every blueprint rejects the query', () => {
    renderOverview([
      failed(rejection('Unknown field color')),
      failed(rejection('Unknown field color on brands')),
      failed(rejection('Unknown field color on colors')),
    ]);

    expect(screen.getByRole('alert').textContent).toContain(
      'Unknown field color',
    );
    expect(screen.queryByText('No records matched this search.')).toBeNull();
  });

  it('keeps loading instead of reporting no matches while a query is pending', () => {
    renderOverview([withItems(0), pending, withItems(0)]);

    expect(screen.getByLabelText('Loading records...')).toBeTruthy();
    expect(screen.queryByText('No records matched this search.')).toBeNull();
  });
});
