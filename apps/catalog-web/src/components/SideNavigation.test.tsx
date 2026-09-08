// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { ComponentPropsWithoutRef, ElementType, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../i18n';
import { currentSession } from '../features/auth/api';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { SideNavigation } from './SideNavigation';

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
          pathname: '/',
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
  ExtensionOutlet: () => null,
}));

const renderNavigation = (onNavigate = vi.fn()) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  render(
    <QueryClientProvider client={client}>
      <SideNavigation onNavigate={onNavigate} />
    </QueryClientProvider>,
  );

  return onNavigate;
};

beforeEach(() => {
  vi.mocked(currentSession).mockResolvedValue(null);
  vi.mocked(listSidebarExploreNavigation).mockResolvedValue([
    { blueprint_code: 'products', blueprint_name: 'Products' },
  ]);
});

describe('SideNavigation', () => {
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
