// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Session } from './api';
import { authQueryKeys } from './queryKeys';
import { useCan } from './useCan';

vi.mock('./api', () => ({ currentSession: () => new Promise(() => {}) }));

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
