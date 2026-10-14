// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Session } from './api';
import { authQueryKeys } from './queryKeys';
import { RequireCapability } from './RequireCapability';

vi.mock('./api', () => ({ currentSession: () => new Promise(() => {}) }));

const renderGate = (session?: Session | null) => {
  const client = new QueryClient();
  if (session !== undefined)
    client.setQueryData(authQueryKeys.session(), session);
  render(
    <QueryClientProvider client={client}>
      <RequireCapability capability="agents_run">
        <p>Agents page</p>
      </RequireCapability>
    </QueryClientProvider>,
  );
};

const session = (agents_run: boolean) =>
  ({ capabilities: { agents_run } }) as Session;

describe('RequireCapability', () => {
  it('renders the page for a caller holding the capability', () => {
    renderGate(session(true));
    expect(screen.getByText('Agents page')).toBeTruthy();
  });

  it('refuses a caller without the capability', () => {
    renderGate(session(false));
    expect(screen.queryByText('Agents page')).toBeNull();
    expect(screen.getByRole('alert').textContent).toBe(
      'You are not authorized to use agents in this workspace.',
    );
  });

  it('renders nothing while the session loads', () => {
    renderGate();
    expect(screen.queryByText('Agents page')).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
