// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import type { ComponentPropsWithoutRef, ElementType, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { listBlueprints, type Blueprint } from '../blueprints/api';
import { WorkspaceOnboardingGate } from './WorkspaceOnboardingGate';

vi.mock('@tanstack/react-router', async () => {
  const { createElement, forwardRef } = await import('react');
  const Link = forwardRef<
    HTMLAnchorElement,
    ComponentPropsWithoutRef<'a'> & { children: ReactNode; to: string }
  >(({ children, to, ...props }, ref) => (
    <a {...props} href={to} ref={ref}>
      {children}
    </a>
  ));
  return {
    createLink: <T,>(Component: T) =>
      forwardRef((props, ref) =>
        createElement(Component as ElementType, {
          ...props,
          component: Link,
          ref,
        }),
      ),
  };
});

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('../blueprints/api', () => ({ listBlueprints: vi.fn() }));

type Session = Awaited<ReturnType<typeof currentSession>>;

const session = (capabilities: Record<string, boolean>) =>
  ({
    capabilities,
    display_name: null,
    email: 'admin@example.test',
    login_identifier: 'acme',
    user_id: '00000000-0000-4000-8000-000000000001',
    workspace_id: '00000000-0000-4000-8000-000000000002',
  }) as Session;

const renderGate = (disabled = false) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <WorkspaceOnboardingGate disabled={disabled}>
        <p>Explorer</p>
      </WorkspaceOnboardingGate>
    </QueryClientProvider>,
  );
};

beforeEach(() => {
  vi.mocked(currentSession).mockResolvedValue(
    session({ blueprints_write: true, members_manage: true }),
  );
  vi.mocked(listBlueprints).mockResolvedValue([]);
});

describe('WorkspaceOnboardingGate', () => {
  it('guides blueprint authors through setup when no blueprints exist', async () => {
    renderGate();

    expect(
      await screen.findByRole('heading', { name: 'Welcome to Attricat' }),
    ).toBeTruthy();
    expect(
      screen
        .getByRole('link', { name: 'Create blueprint' })
        .getAttribute('href'),
    ).toBe('/manage/blueprints/new');
    expect(screen.getByRole('link', { name: 'Invite people' })).toBeTruthy();
    expect(
      screen.queryByRole('link', { name: 'Browse extensions' }),
    ).toBeNull();
    expect(
      screen
        .getByRole('link', {
          name: 'Read the guide: Create your first blueprint',
        })
        .getAttribute('href'),
    ).toBe('https://docs.attricat.com/builders/blueprints/');
    expect(
      screen.getByRole('link', { name: 'Read the documentation' }),
    ).toBeTruthy();
    expect(screen.queryByText('Explorer')).toBeNull();
  });

  it('points other members to an administrator and the documentation', async () => {
    vi.mocked(currentSession).mockResolvedValue(
      session({ blueprints_write: false }),
    );
    renderGate();

    expect(
      await screen.findByRole('heading', { name: 'No blueprints yet' }),
    ).toBeTruthy();
    expect(screen.queryByRole('link', { name: 'Create blueprint' })).toBeNull();
    expect(
      screen.getByRole('link', { name: 'Read the documentation' }),
    ).toBeTruthy();
  });

  it('renders the explorer once a blueprint exists', async () => {
    vi.mocked(listBlueprints).mockResolvedValue([{} as Blueprint]);
    renderGate();

    expect(await screen.findByText('Explorer')).toBeTruthy();
  });

  it('renders the explorer when blueprints cannot be loaded', async () => {
    vi.mocked(listBlueprints).mockRejectedValue(new Error('forbidden'));
    renderGate();

    expect(await screen.findByText('Explorer')).toBeTruthy();
  });

  it('skips the check when disabled', () => {
    renderGate(true);

    expect(screen.getByText('Explorer')).toBeTruthy();
    expect(listBlueprints).not.toHaveBeenCalled();
  });
});
