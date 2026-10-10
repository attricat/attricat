// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render as renderElement, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { ReactElement, ReactNode } from 'react';
import type { Blueprint, BlueprintWithAttributes } from '../records/api';
import { ExplorerSearchForm } from './ExplorerSearchForm';

const revisions = [
  {
    code: 'product',
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: 'Product',
    status: 'published',
    version: 3,
    views: {},
  },
  {
    code: 'product',
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: 'Product',
    status: 'published',
    version: 2,
    views: {},
  },
] satisfies Blueprint[];

const render = (element: ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return renderElement(element, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  });
};

describe('ExplorerSearchForm version scope', () => {
  it('shows the blueprint selector when the route is not locked', () => {
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{}}
      />,
    );

    expect(
      screen.getByRole('combobox', { name: /select a blueprint/i }),
    ).toBeTruthy();
  });

  it('applies a blueprint choice immediately', async () => {
    const onBlueprintChange = vi.fn();
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={[
          revisions[0],
          { ...revisions[0], code: 'brand', name: 'Brand' },
        ]}
        currentVersion={3}
        onBlueprintChange={onBlueprintChange}
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );

    await user.click(
      screen.getByRole('combobox', { name: /select a blueprint/i }),
    );
    await user.click(screen.getByRole('option', { name: /Brand/ }));
    expect(onBlueprintChange).toHaveBeenCalledWith('brand');
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('offers a context choice only beside the default context', async () => {
    const onContextChange = vi.fn();
    const user = userEvent.setup();
    const context = {
      code: 'default',
      data: {},
      id: '123e4567-e89b-12d3-a456-426614174001',
      parent_id: null,
    };
    const { rerender } = render(
      <ExplorerSearchForm
        blueprints={revisions}
        contexts={[context]}
        currentVersion={3}
        lockedBlueprint
        onContextChange={onContextChange}
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );
    await user.click(screen.getByRole('button', { name: /^Search scope/ }));
    expect(screen.queryByRole('combobox', { name: 'Context' })).toBeNull();
    await user.keyboard('{Escape}');

    rerender(
      <ExplorerSearchForm
        blueprints={revisions}
        contexts={[context, { ...context, code: 'storefront', id: 'store' }]}
        currentVersion={3}
        lockedBlueprint
        onContextChange={onContextChange}
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );
    await user.click(
      screen.getByRole('button', {
        name: 'Search scope: Current — v3, Default context',
      }),
    );
    await user.click(screen.getByRole('combobox', { name: 'Context' }));
    await user.click(screen.getByRole('option', { name: 'storefront' }));
    expect(onContextChange).toHaveBeenCalledWith('storefront');
  });

  it('ties the scope button to the popover it opens', async () => {
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );
    const scopeButton = screen.getByRole('button', { name: /^Search scope/ });
    expect(scopeButton.getAttribute('aria-expanded')).toBe('false');
    expect(scopeButton.hasAttribute('aria-controls')).toBe(false);

    await user.click(scopeButton);

    expect(scopeButton.getAttribute('aria-expanded')).toBe('true');
    const popover = document.getElementById(
      scopeButton.getAttribute('aria-controls') ?? '',
    );
    expect(
      popover?.contains(
        screen.getByRole('combobox', { name: 'Version scope' }),
      ),
    ).toBe(true);
  });

  it('marks the scope button when the scope differs from the defaults', () => {
    const scopeButton = () =>
      screen.getByRole('button', { name: /^Search scope/ });
    const { rerender } = render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );
    expect(scopeButton().querySelector('.MuiBadge-invisible')).toBeTruthy();

    rerender(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product', version: 3 }}
      />,
    );
    expect(scopeButton().querySelector('.MuiBadge-invisible')).toBeTruthy();

    rerender(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{ blueprint: 'product', version: 2 }}
      />,
    );
    expect(scopeButton().getAttribute('aria-label')).toBe(
      'Search scope: Version 2',
    );
    expect(scopeButton().querySelector('.MuiBadge-dot')).toBeTruthy();
    expect(scopeButton().querySelector('.MuiBadge-invisible')).toBeNull();
  });

  it('defaults to current and searches immediately for a historical version', async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );

    await user.click(
      screen.getByRole('button', { name: 'Search scope: Current — v3' }),
    );
    expect(
      screen.getByRole('combobox', { name: 'Version scope' }).textContent,
    ).toContain('Current — v3');
    await user.click(screen.getByRole('combobox', { name: 'Version scope' }));
    await user.click(screen.getByRole('option', { name: 'Version 2' }));

    expect(onSubmit).toHaveBeenCalledOnce();
    expect(onSubmit).toHaveBeenCalledWith({
      blueprint: 'product',
      version: 2,
      query: undefined,
    });
  });

  it('opens search syntax from the query field help button', async () => {
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        onSubmit={vi.fn()}
        revisions={revisions}
        search={{}}
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Search syntax' }));

    expect(screen.getByText(/This blueprint: term/)).toBeTruthy();
    const guide = screen.getByRole('link', {
      name: 'Read the search syntax guide',
    });
    expect(guide.getAttribute('href')).toBe(
      'https://docs.attricat.com/guides/search-syntax/',
    );
    expect(guide.getAttribute('target')).toBe('_blank');
    expect(guide.getAttribute('rel')).toBe('noopener noreferrer');
  });

  it('submits all versions as the explicit all-version scope', async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <ExplorerSearchForm
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product', allVersions: true }}
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Search' }));
    expect(onSubmit).toHaveBeenCalledWith({
      blueprint: 'product',
      allVersions: true,
      query: undefined,
    });
  });
});

const productSchema = {
  blueprint: revisions[0],
  attributes: [
    { code: 'sku', value_type: 'string', name: 'SKU' },
    { code: 'title', value_type: 'string' },
  ],
  table_path_attributes: [],
} satisfies BlueprintWithAttributes;

describe('ExplorerSearchForm query autocomplete', () => {
  beforeAll(() => {
    // jsdom does not implement scrolling the active option into view.
    Element.prototype.scrollIntoView = vi.fn();
  });

  const renderQuery = (onSubmit = vi.fn()) =>
    render(
      <ExplorerSearchForm
        blueprintSchema={productSchema}
        blueprints={revisions}
        currentVersion={3}
        lockedBlueprint
        onSubmit={onSubmit}
        revisions={revisions}
        search={{ blueprint: 'product' }}
      />,
    );

  it('lists fields with descriptions and completes the first with Tab', async () => {
    const user = userEvent.setup();
    renderQuery();
    const input = screen.getByRole('combobox', { name: 'Query' });

    await user.type(input, 's');
    const option = screen.getByRole('option', { name: /^sku/ });
    expect(option.textContent).toContain('Match only in SKU');
    expect(option.textContent).toContain('Text');

    await user.keyboard('{Tab}');
    expect((input as HTMLInputElement).value).toBe('sku:');
    expect(screen.getByText(/Type a value to find in sku/)).toBeTruthy();
  });

  it('submits on Enter unless a suggestion was chosen with the arrows', async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    renderQuery(onSubmit);
    const input = screen.getByRole('combobox', { name: 'Query' });

    await user.type(input, 'ti{ArrowDown}{Enter}');
    expect((input as HTMLInputElement).value).toBe('title:');
    expect(onSubmit).not.toHaveBeenCalled();

    await user.type(input, 'linen{Enter}');
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ query: 'title:linen' }),
    );
  });

  it('explains unknown fields', async () => {
    const user = userEvent.setup();
    renderQuery();

    await user.type(
      screen.getByRole('combobox', { name: 'Query' }),
      'colour:red{Escape}',
    );
    await user.tab();
    expect(
      screen.getByText('“colour” is not a searchable field.'),
    ).toBeTruthy();
  });
});
