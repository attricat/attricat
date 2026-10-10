// @vitest-environment jsdom
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { UnsavedFieldChangesGuard } from './UnsavedFieldChangesGuard';

const renderGuarded = () => {
  const rootRoute = createRootRoute({ component: Outlet });
  const explorerRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    validateSearch: (search: Record<string, unknown>) =>
      search as { record?: string; q?: string },
    component: UnsavedFieldChangesGuard,
  });
  const otherRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/other',
  });
  const router = createRouter({
    history: createMemoryHistory({ initialEntries: ['/?record=a'] }),
    routeTree: rootRoute.addChildren([explorerRoute, otherRoute]),
  });
  render(<RouterProvider router={router} />);
  return router;
};

describe('UnsavedFieldChangesGuard', () => {
  it('asks before leaving the record and stays when told to', async () => {
    const user = userEvent.setup();
    const router = renderGuarded();
    await waitFor(() => expect(router.state.status).toBe('idle'));

    act(() => router.history.push('/?record=b'));
    await user.click(
      await screen.findByRole('button', { name: 'records.keepEditing' }),
    );

    expect(router.state.location.search).toEqual({ record: 'a' });
  });

  it('leaves once the user discards the changes', async () => {
    const user = userEvent.setup();
    const router = renderGuarded();
    await waitFor(() => expect(router.state.status).toBe('idle'));

    act(() => router.history.push('/other'));
    await user.click(
      await screen.findByRole('button', { name: 'records.discardAndLeave' }),
    );

    await waitFor(() => expect(router.state.location.pathname).toBe('/other'));
  });

  it('lets search changes that keep the record through', async () => {
    const router = renderGuarded();
    await waitFor(() => expect(router.state.status).toBe('idle'));

    act(() => router.history.push('/?record=a&q=shirts'));
    await waitFor(() =>
      expect(router.state.location.search).toEqual({
        record: 'a',
        q: 'shirts',
      }),
    );
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
