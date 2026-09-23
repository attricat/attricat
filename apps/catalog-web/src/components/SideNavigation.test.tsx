// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type {
  ComponentProps,
  ComponentPropsWithoutRef,
  ElementType,
  ReactNode,
} from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../i18n';
import { currentSession } from '../features/auth/api';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { SideNavigation } from './SideNavigation';

let currentPathname = '/';

vi.mock('@tanstack/react-router', async () => {
  const { createElement, forwardRef } = await import('react');

  const Link = forwardRef<
    HTMLAnchorElement,
    ComponentPropsWithoutRef<'a'> & {
      children: ReactNode;
      search?: Record<string, string | boolean>;
      to: string;
    }
  >(({ children, onClick, search, to, ...props }, ref) => {
    const query = new URLSearchParams(
      Object.entries(search ?? {}).map(([key, value]) => [key, String(value)]),
    );
    const href = query.size ? `${to}?${query}` : to;

    return (
      <a
        {...props}
        href={href}
        onClick={(event) => {
          event.preventDefault();
          onClick?.(event);
        }}
        ref={ref}
      >
        {children}
      </a>
    );
  });

  return {
    createLink: <T,>(Component: T) =>
      forwardRef((props, ref) =>
        createElement(Component as ElementType, {
          ...props,
          component: Link,
          ref,
        }),
      ),
    Link,
    useRouterState: ({
      select,
    }: {
      select: (state: {
        location: {
          pathname: string;
          search: { blueprint: string; locked: boolean };
        };
      }) => unknown;
    }) =>
      select({
        location: {
          pathname: currentPathname,
          search: { blueprint: 'products', locked: true },
        },
      }),
  };
});

vi.mock('../features/auth/api', () => ({
  currentSession: vi.fn(),
}));

vi.mock('../features/workspace/api', () => ({
  listSidebarExploreNavigation: vi.fn(),
}));

vi.mock('../features/extensions/ExtensionOutlet', () => ({
  ExtensionOutlet: ({
    navigationDisplay,
    onBrowseExtensions,
    onNavigate,
  }: {
    navigationDisplay?: string;
    onBrowseExtensions?: () => void;
    onNavigate?: () => void;
  }) =>
    navigationDisplay === 'all' ? (
      <>
        <span>Extension routes</span>
        <a
          href="/manage/extensions"
          onClick={(event) => {
            event.preventDefault();
            onBrowseExtensions?.();
            onNavigate?.();
          }}
        >
          Browse extensions
        </a>
      </>
    ) : null,
}));

const renderNavigation = (
  onNavigate = vi.fn(),
  props: Partial<ComponentProps<typeof SideNavigation>> = {},
) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  render(
    <QueryClientProvider client={client}>
      <SideNavigation {...props} onNavigate={onNavigate} />
    </QueryClientProvider>,
  );

  return onNavigate;
};

beforeEach(() => {
  currentPathname = '/';
  vi.mocked(currentSession).mockResolvedValue(null);
  vi.mocked(listSidebarExploreNavigation).mockResolvedValue([
    { blueprint_code: 'products', blueprint_name: 'Products' },
  ]);
});

describe('SideNavigation', () => {
  it('replaces the mobile primary navigation with sub-navigation and supports going back', async () => {
    const onNavigate = renderNavigation();

    expect(
      screen.getByRole('heading', { name: /entity explorer/i }),
    ).toBeTruthy();
    expect(screen.getByRole('link', { name: 'All entities' })).toBeTruthy();
    expect(await screen.findByRole('link', { name: 'Products' })).toBeTruthy();
    expect(screen.queryByRole('link', { name: 'Agents' })).toBeNull();

    fireEvent.click(
      screen.getByRole('button', { name: 'Back to main navigation' }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Manage' }));

    expect(screen.getByRole('heading', { name: 'Manage' })).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Back to main navigation' }),
    ).toBeTruthy();
    expect(screen.queryByRole('link', { name: 'Agents' })).toBeNull();
    expect(
      await screen.findByRole('link', { name: 'Blueprints' }),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole('button', { name: 'Back to main navigation' }),
    );
    fireEvent.click(screen.getByRole('link', { name: 'Apps' }));

    expect(screen.getByRole('heading', { name: 'Apps' })).toBeTruthy();
    expect(screen.getByText('Extension routes')).toBeTruthy();
    expect(screen.queryByRole('link', { name: 'Agents' })).toBeNull();
    fireEvent.click(screen.getByRole('link', { name: 'Browse extensions' }));
    expect(onNavigate).toHaveBeenCalledOnce();

    fireEvent.click(
      screen.getByRole('button', { name: 'Back to main navigation' }),
    );

    expect(screen.getByRole('link', { name: 'Agents' })).toBeTruthy();
    expect(screen.queryByRole('link', { name: 'Blueprints' })).toBeNull();
  });

  it('keeps compact Explore and Manage panels mutually exclusive', async () => {
    const onCompactManageOpenChange = vi.fn();
    renderNavigation(vi.fn(), {
      compact: true,
      onCompactManageOpenChange,
    });

    fireEvent.click(screen.getByRole('link', { name: 'Manage' }));
    onCompactManageOpenChange.mockClear();
    fireEvent.click(screen.getByRole('link', { name: /entity explorer/i }));

    expect(onCompactManageOpenChange).toHaveBeenCalledOnce();
    expect(onCompactManageOpenChange).toHaveBeenCalledWith(false);
    expect(screen.queryByRole('navigation', { name: 'Manage' })).toBeNull();
    expect(
      screen.getByRole('navigation', { name: /entity explorer/i }),
    ).toBeTruthy();
  });

  it('follows route changes in the mobile drawer', () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const view = render(
      <QueryClientProvider client={client}>
        <SideNavigation />
      </QueryClientProvider>,
    );
    expect(
      screen.getByRole('heading', { name: /entity explorer/i }),
    ).toBeTruthy();

    currentPathname = '/manage/extensions';
    view.rerender(
      <QueryClientProvider client={client}>
        <SideNavigation />
      </QueryClientProvider>,
    );
    expect(screen.getByRole('heading', { name: 'Manage' })).toBeTruthy();

    currentPathname = '/extensions';
    view.rerender(
      <QueryClientProvider client={client}>
        <SideNavigation />
      </QueryClientProvider>,
    );
    expect(screen.getByRole('heading', { name: 'Apps' })).toBeTruthy();
  });

  it('switches from the compact Apps pane to Manage when browsing extensions', () => {
    renderNavigation(vi.fn(), { compact: true });
    fireEvent.click(screen.getByRole('link', { name: 'Apps' }));
    fireEvent.click(screen.getByRole('link', { name: 'Browse extensions' }));

    expect(screen.queryByRole('navigation', { name: 'Apps' })).toBeNull();
    expect(screen.getByRole('navigation', { name: 'Manage' })).toBeTruthy();
  });

  it('closes the compact Apps panel when navigating to Profile', () => {
    renderNavigation(vi.fn(), { compact: true });
    fireEvent.click(screen.getByRole('link', { name: 'Apps' }));
    expect(screen.getByRole('navigation', { name: 'Apps' })).toBeTruthy();
    fireEvent.click(screen.getByRole('link', { name: 'Profile' }));
    expect(screen.queryByRole('navigation', { name: 'Apps' })).toBeNull();
  });

  it('renders a pinned Explore item as a selected link without a nested button', async () => {
    const onNavigate = renderNavigation();
    const productsLink = await screen.findByRole('link', { name: 'Products' });

    expect(productsLink.getAttribute('href')).toBe(
      '/?blueprint=products&locked=true',
    );
    expect(productsLink.className).toContain('Mui-selected');
    expect(productsLink.querySelector('a')).toBeNull();
    expect(within(productsLink).queryByRole('button')).toBeNull();

    fireEvent.click(productsLink);

    expect(onNavigate).toHaveBeenCalledOnce();
  });
});
