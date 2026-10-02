// @vitest-environment jsdom
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Session } from './api';
import { authQueryKeys } from './queryKeys';
import { useSessionCacheBoundary } from './useSessionCacheBoundary';

const session = { user_id: 'user-a', workspace_id: 'workspace-a' } as Session;

const Boundary = ({ children }: { children: React.ReactNode }) => {
  const query = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: async () => session,
    staleTime: Infinity,
  });
  const boundary = useSessionCacheBoundary(query.data, query.isSuccess);
  return boundary.ready && query.data ? (
    <div key={boundary.identity}>{children}</div>
  ) : null;
};

describe('session cache boundary', () => {
  it.each([
    ['another user', { ...session, user_id: 'user-b' }],
    ['another workspace', { ...session, workspace_id: 'workspace-b' }],
    ['an expired session', null],
  ])('removes protected data before rendering %s', async (_, next) => {
    const client = new QueryClient();
    client.setQueryData(authQueryKeys.session(), session);
    const reads: unknown[] = [];
    const Protected = () => {
      reads.push(client.getQueryData(['protected']));
      return <div>Protected page</div>;
    };
    render(
      <QueryClientProvider client={client}>
        <Boundary>
          <Protected />
        </Boundary>
      </QueryClientProvider>,
    );
    await screen.findByText('Protected page');
    client.setQueryData(['protected'], 'old account data');
    client
      .getMutationCache()
      .build(client, { mutationFn: async () => 'secret' });
    reads.length = 0;
    act(() => {
      client.setQueryData(authQueryKeys.session(), next);
    });
    await waitFor(() =>
      expect(client.getQueryData(['protected'])).toBeUndefined(),
    );
    expect(client.getMutationCache().getAll()).toHaveLength(0);
    expect(client.getQueryData(authQueryKeys.session())).toEqual(next);
    expect(reads).not.toContain('old account data');
    if (next === null) expect(screen.queryByText('Protected page')).toBeNull();
  });

  it('does not clear data when only preferences change', async () => {
    const client = new QueryClient();
    client.setQueryData(authQueryKeys.session(), session);
    render(
      <QueryClientProvider client={client}>
        <Boundary>
          <div>Protected page</div>
        </Boundary>
      </QueryClientProvider>,
    );
    await screen.findByText('Protected page');
    client.setQueryData(['protected'], 'same account data');
    const remove = vi.spyOn(client, 'removeQueries');
    await act(async () => {
      client.setQueryData(authQueryKeys.session(), {
        ...session,
        time_zone: 'Europe/Warsaw',
      });
    });
    expect(remove).not.toHaveBeenCalled();
    expect(client.getQueryData(['protected'])).toBe('same account data');
  });

  it('cancels in-flight protected requests on identity change', async () => {
    const client = new QueryClient();
    client.setQueryData(authQueryKeys.session(), session);
    render(
      <QueryClientProvider client={client}>
        <Boundary>
          <div>Protected page</div>
        </Boundary>
      </QueryClientProvider>,
    );
    await screen.findByText('Protected page');
    let signal: AbortSignal | undefined;
    const pending = client
      .fetchQuery({
        queryKey: ['protected'],
        queryFn: (context) => {
          signal = context.signal;
          return new Promise(() => {});
        },
      })
      .catch(() => undefined);
    act(() => {
      client.setQueryData(authQueryKeys.session(), null);
    });
    await waitFor(() => expect(signal?.aborted).toBe(true));
    await pending;
    expect(client.getQueryData(['protected'])).toBeUndefined();
  });
});
