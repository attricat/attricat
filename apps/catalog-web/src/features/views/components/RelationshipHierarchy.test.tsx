// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import '../../../i18n';
import { getEntityHierarchy } from '../../entities/api';
import { RelationshipHierarchy } from './RelationshipHierarchy';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('../../entities/api', () => ({ getEntityHierarchy: vi.fn() }));

it('does not show a permanent loading state when no context is selected', () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <RelationshipHierarchy
        attribute={{ code: 'parent' }}
        entityId="entity-1"
        value={null}
      />
    </QueryClientProvider>,
  );
  expect(screen.queryByText('Loading hierarchy…')).toBeNull();
  expect(screen.getByText('No hierarchy available.')).toBeTruthy();
  expect(getEntityHierarchy).not.toHaveBeenCalled();
});
