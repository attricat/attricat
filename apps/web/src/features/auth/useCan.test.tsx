// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Session } from './api';
import { authQueryKeys } from './queryKeys';
import { useAccess, useCan } from './useCan';

const checkAccess = vi.hoisted(() => vi.fn());

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  checkAccess,
  currentSession: () => new Promise(() => {}),
}));

const render = (session: Session | null | undefined) => {
  const client = new QueryClient();
  if (session !== undefined)
    client.setQueryData(authQueryKeys.session(), session);
  return renderHook(() => useCan('records_delete'), {
    wrapper: ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  }).result.current;
};

const session = (capabilities?: Partial<Session['capabilities']>) =>
  ({ user_id: 'user-a', capabilities }) as Session;

describe('useCan', () => {
  it('reports a granted capability', () => {
    expect(render(session({ records_delete: true }))).toBe(true);
  });

  it.each([
    ['a refused capability', session({ records_delete: false })],
    ['a session without capabilities', session()],
    ['a signed-out caller', null],
    ['a session still loading', undefined],
  ])('denies %s', (_, value) => {
    expect(render(value)).toBe(false);
  });
});

describe('useAccess', () => {
  const [first, second] = [
    '00000000-0000-4000-8000-000000000001',
    '00000000-0000-4000-8000-000000000002',
  ];
  const renderAccess = (value: Session) => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    client.setQueryData(authQueryKeys.session(), value);
    return renderHook(
      () => useAccess('records_delete', 'record', [second, first, second]),
      {
        wrapper: ({ children }) => (
          <QueryClientProvider client={client}>{children}</QueryClientProvider>
        ),
      },
    ).result;
  };
  beforeEach(() => checkAccess.mockReset());

  it('grants every target from a workspace-wide capability without asking', () => {
    const result = renderAccess(session({ records_delete: true }));
    expect(result.current.isPending).toBe(false);
    expect(result.current.can(first)).toBe(true);
    expect(result.current.can(undefined)).toBe(false);
    expect(checkAccess).not.toHaveBeenCalled();
  });

  it('asks the server once for the targets of a scoped capability', async () => {
    checkAccess.mockResolvedValue({ granted: [second] });
    const result = renderAccess(session({ records_delete: false }));
    expect(result.current.isPending).toBe(true);
    expect(result.current.can(second)).toBe(false);
    await waitFor(() => expect(result.current.isPending).toBe(false));
    expect(result.current.can(second)).toBe(true);
    expect(result.current.can(first)).toBe(false);
    expect(checkAccess).toHaveBeenCalledExactlyOnceWith(
      'records.delete',
      'record',
      [first, second],
    );
  });
});
